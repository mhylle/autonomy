use clap::Parser;
use crossbeam_channel::{Receiver, unbounded};
use tracing::info;

use simulation_engine::core::config::SimulationConfig;
use simulation_engine::core::perf::PerformanceStats;
use simulation_engine::core::tick;
use simulation_engine::core::world::SimulationWorld;
use simulation_engine::net::bridge;
use simulation_engine::net::server::{ServerState, ViewerCommand};

/// Autonomy: emergent life simulation engine
#[derive(Parser, Debug)]
#[command(name = "autonomy", version, about)]
struct Cli {
    /// Master seed for deterministic simulation
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// World width in simulation units
    #[arg(long, default_value_t = 500.0)]
    world_width: f64,

    /// World height in simulation units
    #[arg(long, default_value_t = 500.0)]
    world_height: f64,

    /// Number of entities to spawn at start
    #[arg(long, default_value_t = 100)]
    initial_entities: u32,

    /// Target ticks per second
    #[arg(long, default_value_t = 60)]
    tick_rate: u32,

    /// Run without viewer/network (headless mode)
    #[arg(long, default_value_t = false)]
    headless: bool,

    /// Path to JSON config file (overrides CLI args)
    #[arg(long)]
    config: Option<String>,

    /// WebSocket server port
    #[arg(long, default_value_t = 9001)]
    port: u16,

    /// Snapshot interval in ticks (0 = disabled)
    #[arg(long, default_value_t = 1000)]
    snapshot_interval: u64,

    /// Directory for snapshot files
    #[arg(long, default_value = "snapshots")]
    snapshot_dir: String,

    /// Replay from a snapshot file instead of starting fresh
    #[arg(long)]
    replay: Option<String>,

    /// Target tick to replay to (requires --replay)
    #[arg(long)]
    to_tick: Option<u64>,

    /// Show per-system performance statistics every N ticks
    #[arg(long)]
    show_perf: bool,

    /// Performance stats logging interval in ticks
    #[arg(long, default_value_t = 100)]
    perf_interval: u64,

    /// Maximum number of concurrent WebSocket viewer connections
    #[arg(long, default_value_t = 8)]
    max_clients: usize,
}

fn main() {
    init_logging();

    let cli = Cli::parse();

    // ---- Replay mode ----
    if let Some(snapshot_path) = &cli.replay {
        return run_replay(snapshot_path, cli.to_tick);
    }

    let config = build_config(&cli);
    let port = cli.port;
    let show_perf = cli.show_perf;
    let perf_interval = cli.perf_interval;
    let max_clients = cli.max_clients;

    info!(
        seed = config.seed,
        width = config.world_width,
        height = config.world_height,
        entities = config.initial_entity_count,
        tick_rate = config.tick_rate,
        headless = config.headless,
        snapshot_interval = config.snapshot_interval,
        show_perf = show_perf,
        max_clients = max_clients,
        "starting simulation"
    );

    let mut world = SimulationWorld::new(config);
    simulation_engine::environment::spawning::scatter_resources(&mut world);
    simulation_engine::core::spawning::spawn_initial_population(&mut world);

    let (command_tx, command_rx) = unbounded();
    let server_state = ServerState::new_with_client_limit(command_tx, max_clients);

    // Send initial snapshot so early-connecting clients get state.
    bridge::update_snapshot(&world, &server_state);

    if !world.config.headless {
        let server_state_clone = server_state.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(simulation_engine::net::server::start_server(
                server_state_clone,
                port,
            ));
        });
    }

    let mut perf_stats = if show_perf {
        let mut stats = PerformanceStats::new(true);
        stats.log_interval = perf_interval;
        Some(stats)
    } else {
        None
    };

    run_loop(&mut world, &server_state, &command_rx, &mut perf_stats);
}

fn run_replay(snapshot_path: &str, to_tick: Option<u64>) {
    use simulation_engine::core::snapshot;

    info!(path = snapshot_path, "loading snapshot for replay");

    let mut world = snapshot::load_snapshot(std::path::Path::new(snapshot_path))
        .unwrap_or_else(|e| panic!("failed to load snapshot '{}': {}", snapshot_path, e));

    let start_tick = world.tick;
    let target_tick = to_tick.unwrap_or(start_tick);

    if target_tick < start_tick {
        panic!(
            "target tick {} is before snapshot tick {}",
            target_tick, start_tick
        );
    }

    info!(
        start_tick = start_tick,
        target_tick = target_tick,
        ticks_to_replay = target_tick - start_tick,
        entities = world.entity_count(),
        "replaying simulation"
    );

    while world.tick < target_tick {
        tick::tick(&mut world);

        if world.tick % 1000 == 0 {
            info!(
                tick = world.tick,
                entities = world.entity_count(),
                "replay progress"
            );
        }
    }

    info!(
        tick = world.tick,
        entities = world.entity_count(),
        "replay complete"
    );
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
}

fn build_config(cli: &Cli) -> SimulationConfig {
    if let Some(path) = &cli.config {
        load_config_file(path)
    } else {
        config_from_cli(cli)
    }
}

fn load_config_file(path: &str) -> SimulationConfig {
    let contents = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read config file '{}': {}", path, e));
    serde_json::from_str(&contents)
        .unwrap_or_else(|e| panic!("failed to parse config file '{}': {}", path, e))
}

fn config_from_cli(cli: &Cli) -> SimulationConfig {
    SimulationConfig {
        world_width: cli.world_width,
        world_height: cli.world_height,
        seed: cli.seed,
        initial_entity_count: cli.initial_entities,
        tick_rate: cli.tick_rate,
        headless: cli.headless,
        snapshot_interval: cli.snapshot_interval,
        snapshot_dir: cli.snapshot_dir.clone(),
        enable_3d: false,
        enable_chunks: false,
        chunk_size: 256.0,
    }
}

/// Max broadcast rate to viewers (Hz). At high sim speeds we skip
/// intermediate ticks to avoid overwhelming the WebSocket/browser.
/// 15Hz is sufficient for smooth visual updates (canvas re-renders
/// at requestAnimationFrame rate regardless).
const MAX_BROADCAST_HZ: f64 = 15.0;

fn run_loop(
    world: &mut SimulationWorld,
    server_state: &ServerState,
    command_rx: &Receiver<ViewerCommand>,
    perf_stats: &mut Option<PerformanceStats>,
) {
    let base_tick_duration = std::time::Duration::from_secs_f64(1.0 / world.config.tick_rate as f64);
    let min_broadcast_interval = std::time::Duration::from_secs_f64(1.0 / MAX_BROADCAST_HZ);
    let mut last_broadcast = std::time::Instant::now();
    let mut diff_engine = simulation_engine::net::diff::DiffEngine::new();

    loop {
        let start = std::time::Instant::now();

        // Drain all pending commands.
        while let Ok(cmd) = command_rx.try_recv() {
            match cmd {
                ViewerCommand::Pause => {
                    world.paused = true;
                    info!("simulation paused");
                }
                ViewerCommand::Resume => {
                    world.paused = false;
                    info!("simulation resumed");
                }
                ViewerCommand::SetSpeed(speed) => {
                    world.speed_multiplier = speed.clamp(0.1, 100.0);
                    info!(speed = world.speed_multiplier, "speed changed");
                }
                ViewerCommand::SubscribeViewport(bounds) => {
                    if let Ok(mut vp) = server_state.viewport.write() {
                        *vp = bounds;
                    }
                }
            }
        }

        if !world.paused {
            // Read current viewport for LOD computation.
            let viewport = server_state
                .viewport
                .read()
                .map(|v| *v)
                .unwrap_or_default();

            tick::tick_with_perf(world, perf_stats, &viewport);

            // Log performance stats periodically if enabled.
            if let Some(ref stats) = perf_stats {
                if stats.enabled && stats.tick_timing.count > 0
                    && world.tick % stats.log_interval == 0
                {
                    info!("\n{}", stats.summary());
                }
            }

            // Throttle broadcasts: only send at most MAX_BROADCAST_HZ per second
            // to avoid overwhelming the viewer at high sim speeds.
            let now = std::time::Instant::now();
            if now.duration_since(last_broadcast) >= min_broadcast_interval {
                bridge::broadcast_diff_tick(world, server_state, &mut diff_engine);
                last_broadcast = now;
            }

            // Update snapshot periodically for new client connections.
            if world.tick % 100 == 0 {
                bridge::update_snapshot(world, server_state);
            }

            // Periodic disk snapshots for replay.
            let interval = world.config.snapshot_interval;
            if interval > 0 && world.tick % interval == 0 {
                let snap_dir = world.config.snapshot_dir.clone();
                if let Err(e) = simulation_engine::core::snapshot::save_snapshot_to_dir(world, &snap_dir) {
                    tracing::warn!(error = %e, "failed to save snapshot");
                }
            }

            if world.tick % 1000 == 0 {
                info!(
                    tick = world.tick,
                    entities = world.entity_count(),
                    "progress"
                );
            }
        }

        let tick_duration = base_tick_duration.div_f64(world.speed_multiplier);
        let elapsed = start.elapsed();
        if elapsed < tick_duration {
            std::thread::sleep(tick_duration - elapsed);
        }
    }
}
