// Tauri service functions
// This file contains functions for communicating with the Tauri backend

import { invoke } from '@tauri-apps/api/core';

export async function greet(name: string): Promise<string> {
  return await invoke('greet', { name });
}

// Add more Tauri commands here as they are implemented
