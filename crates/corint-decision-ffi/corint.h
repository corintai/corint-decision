/**
 * CORINT Decision Engine - C FFI Header
 *
 * This header file defines the C interface for the CORINT Decision Engine.
 * It can be used to integrate CORINT with C, C++, and other languages that
 * support C FFI.
 */

#ifndef CORINT_FFI_H
#define CORINT_FFI_H

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Opaque handle to a CORINT decision engine
 */
typedef void* CorintEngine;

/**
 * Initialize the logging system
 */
void corint_init_logging(void);

/**
 * Create a new decision engine from a file system repository
 *
 * @param repository_path Path to the repository directory
 * @return Engine handle, or NULL on failure
 */
CorintEngine corint_engine_new(const char* repository_path);

/**
 * Create a new decision engine from a PostgreSQL database
 *
 * @param database_url PostgreSQL connection URL
 * @return Engine handle, or NULL on failure
 */
CorintEngine corint_engine_new_from_database(const char* database_url);

/**
 * Execute a decision using the engine
 *
 * @param engine Engine handle
 * @param request_json JSON-encoded decision request
 * @return JSON-encoded decision response, or NULL on failure
 *         The returned string must be freed with corint_string_free()
 */
char* corint_engine_decide(CorintEngine engine, const char* request_json);

/**
 * Free a decision engine
 *
 * @param engine Engine handle to free
 */
void corint_engine_free(CorintEngine engine);

/**
 * Free a string returned by CORINT FFI functions
 *
 * @param s String to free
 */
void corint_string_free(char* s);

/**
 * Get the CORINT version
 *
 * @return Version string (must be freed with corint_string_free())
 */
char* corint_version(void);

#ifdef __cplusplus
}
#endif

#endif /* CORINT_FFI_H */
