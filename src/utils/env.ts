export function getEnvFlag(name: string, defaultValue = false): boolean {
  try {
    const v = (import.meta as any).env?.[name];
    if (v === 'true') return true;
    if (v === 'false') return false;
    return defaultValue;
  } catch {
    return defaultValue;
  }
}

export const FLAGS = {
  EVENT_SUMMARY_SILENT: () => getEnvFlag('VITE_EVENT_SUMMARY_SILENT', false),
};
