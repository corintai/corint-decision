package com.corint;

import com.sun.jna.Library;
import com.sun.jna.Native;
import com.sun.jna.Pointer;

/**
 * JNA interface to CORINT FFI library
 */
interface CorintNative extends Library {
    CorintNative INSTANCE = Native.load("corint_ffi", CorintNative.class);

    String corint_version();
    void corint_init_logging();
    Pointer corint_engine_new(String repository_path);
    Pointer corint_engine_new_from_database(String database_url);
    String corint_engine_decide(Pointer engine, String request_json);
    void corint_engine_free(Pointer engine);
    void corint_string_free(String s);
}
