type ButterchurnPresetProvider = {
  getPresets?: () => Record<string, unknown>;
};

export async function loadMilkdropPresetMap(): Promise<Record<string, unknown>> {
  const presetModule = await import('butterchurn-presets/lib/butterchurnPresetsMinimal.min.js');
  const presetProvider = (presetModule.default ?? presetModule) as ButterchurnPresetProvider;

  return typeof presetProvider.getPresets === 'function'
    ? presetProvider.getPresets()
    : {};
}

export async function getMilkdropPresetNames(): Promise<string[]> {
  const presetMap = await loadMilkdropPresetMap();
  return Object.keys(presetMap).sort((a, b) => a.localeCompare(b));
}
