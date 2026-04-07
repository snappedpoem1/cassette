export async function getMilkdropPresetNames(): Promise<string[]> {
  const presetModule = await import('butterchurn-presets');
  const presetProvider = (presetModule.default ?? presetModule) as {
    getPresets?: () => Record<string, unknown>;
  };

  const presetMap =
    typeof presetProvider.getPresets === 'function'
      ? presetProvider.getPresets()
      : {};

  return Object.keys(presetMap).sort((a, b) => a.localeCompare(b));
}
