/**
 * CORINT Decision Engine - TypeScript/Node.js bindings
 */

import * as ffi from 'ffi-napi';
import * as path from 'path';
import * as os from 'os';

// Determine library name based on platform
function getLibraryPath(): string {
  const platform = os.platform();
  let libName: string;

  if (platform === 'darwin') {
    libName = 'libcorint_ffi.dylib';
  } else if (platform === 'linux') {
    libName = 'libcorint_ffi.so';
  } else if (platform === 'win32') {
    libName = 'corint_ffi.dll';
  } else {
    throw new Error(`Unsupported platform: ${platform}`);
  }

  // Search in common locations
  const searchPaths = [
    // Development build
    path.join(__dirname, '../../../target/debug', libName),
    path.join(__dirname, '../../../target/release', libName),
    // System install
    path.join('/usr/local/lib', libName),
    path.join('/usr/lib', libName),
  ];

  const fs = require('fs');
  for (const libPath of searchPaths) {
    if (fs.existsSync(libPath)) {
      return libPath;
    }
  }

  throw new Error(`Could not find ${libName}. Please build the FFI library first.`);
}

// Load the library
const lib = ffi.Library(getLibraryPath(), {
  corint_version: ['string', []],
  corint_init_logging: ['void', []],
  corint_engine_new: ['pointer', ['string']],
  corint_engine_new_from_database: ['pointer', ['string']],
  corint_engine_decide: ['string', ['pointer', 'string']],
  corint_engine_free: ['void', ['pointer']],
  corint_string_free: ['void', ['string']],
});

/**
 * Decision request options
 */
export interface DecisionOptions {
  enableTrace?: boolean;
}

/**
 * Decision request
 */
export interface DecisionRequest {
  event_data: Record<string, any>;
  features?: Record<string, any>;
  api?: Record<string, any>;
  service?: Record<string, any>;
  llm?: Record<string, any>;
  vars?: Record<string, any>;
  metadata?: Record<string, string>;
  options?: DecisionOptions;
}

/**
 * Decision response
 */
export interface DecisionResponse {
  decision: string;
  actions: any[];
  trace?: any;
  metadata?: Record<string, any>;
}

/**
 * CORINT Decision Engine
 */
export class DecisionEngine {
  private handle: any;

  /**
   * Create a new decision engine
   * @param options - Configuration options
   */
  constructor(options: { repositoryPath?: string; databaseUrl?: string }) {
    if (!options.repositoryPath && !options.databaseUrl) {
      throw new Error('Either repositoryPath or databaseUrl must be provided');
    }

    if (options.repositoryPath && options.databaseUrl) {
      throw new Error('Cannot specify both repositoryPath and databaseUrl');
    }

    if (options.repositoryPath) {
      this.handle = lib.corint_engine_new(options.repositoryPath);
    } else if (options.databaseUrl) {
      this.handle = lib.corint_engine_new_from_database(options.databaseUrl);
    }

    if (!this.handle || this.handle.isNull()) {
      throw new Error('Failed to create decision engine');
    }
  }

  /**
   * Execute a decision
   * @param request - The decision request
   * @returns The decision response
   */
  decide(request: DecisionRequest): DecisionResponse {
    if (!this.handle || this.handle.isNull()) {
      throw new Error('Engine has been closed');
    }

    // Convert request to JSON
    const requestJson = JSON.stringify(request);

    // Call FFI function
    const resultJson = lib.corint_engine_decide(this.handle, requestJson);

    if (!resultJson) {
      throw new Error('Decision execution failed');
    }

    // Parse response
    const result = JSON.parse(resultJson);

    // Check for errors
    if (result.error) {
      throw new Error(`Decision error: ${result.error}`);
    }

    return result as DecisionResponse;
  }

  /**
   * Close the engine and free resources
   */
  close(): void {
    if (this.handle && !this.handle.isNull()) {
      lib.corint_engine_free(this.handle);
      this.handle = null;
    }
  }

  /**
   * Get the CORINT version
   */
  static version(): string {
    return lib.corint_version();
  }

  /**
   * Initialize the logging system
   */
  static initLogging(): void {
    lib.corint_init_logging();
  }
}
