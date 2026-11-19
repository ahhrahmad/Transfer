// SPDX-License-Identifier: MIT
package com.transfer.fork.server

import android.content.Context
import org.json.JSONObject

/**
 * JNI surface of `transfer-core` (see `transfer-core/src/jni.rs`).
 *
 * Signatures are instance methods on an object, so the native side receives the singleton
 * instance as its second parameter.
 */
object RustServer {

    init {
        System.loadLibrary("transfer_core")
    }

    /** Returns an opaque handle, or throws if the port cannot be bound. */
    external fun start0(configJson: String, callback: SafBridge): Long

    external fun stop0(handle: Long)

    fun configJson(context: Context, port: Int, password: String?): String {
        val assets = JSONObject()
        // Only the web UI; dependency assets (e.g. an empty `geoid_map` dir) are not servable.
        val webAssets = setOf("html", "js", "css", "svg")
        context.assets.list("").orEmpty()
            .filter { it.substringAfterLast('.', "").lowercase() in webAssets }
            .forEach { name ->
                assets.put(
                    name,
                    context.assets.open(name).use { it.readBytes().toString(Charsets.UTF_8) },
                )
            }
        return JSONObject()
            .put("port", port)
            .put("password", password ?: JSONObject.NULL)
            .put("assets", assets)
            .toString()
    }
}
