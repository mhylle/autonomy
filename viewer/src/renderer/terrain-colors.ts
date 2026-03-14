/**
 * Terrain type constants and color mappings.
 *
 * Terrain type integers match the protobuf encoding:
 *   0 = Grassland, 1 = Desert, 2 = Water, 3 = Forest, 4 = Mountain
 */

export const TERRAIN_COLORS: Record<number, string> = {
  0: '#4a7c3f', // Grassland
  1: '#d4a055', // Desert
  2: '#3a6ea5', // Water
  3: '#2d5a1e', // Forest
  4: '#8a8a8a', // Mountain
};

/** Default color for unknown terrain types. */
export const DEFAULT_TERRAIN_COLOR = '#1a1a2e';
