package com.transfer.server

import android.content.Context
import android.net.Uri
import androidx.documentfile.provider.DocumentFile
import com.transfer.util.FileUtils
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject

/**
 * The only Android-specific half of the server: resolves SAF documents and hands raw file
 * descriptors to the Rust core, which owns the sockets.
 */
class SafBridge(
    private val service: FileServerService,
    context: Context,
    treeUri: Uri,
) {
    private val resolver = context.contentResolver
    private val base: DocumentFile =
        DocumentFile.fromTreeUri(context, treeUri) ?: error("Shared folder not accessible")

    /** Called from Rust worker threads. */
    fun listFiles(): String {
        val out = JSONArray()
        base.listFiles().filter { it.isFile && it.canRead() }.forEach { file ->
            out.put(
                JSONObject()
                    .put("name", file.name ?: "unknown")
                    .put("size", file.length())
                    .put("mtime", file.lastModified() / 1000L)
                    .put("mime", file.type ?: "application/octet-stream")
            )
        }
        return out.toString()
    }

    fun openRead(name: String): String {
        val file = base.findFile(name)
        if (file == null || !file.isFile || !file.canRead()) return errorJson("File not found: $name")
        val pfd = resolver.openFileDescriptor(file.uri, "r")
            ?: return errorJson("Could not open file stream.")
        return JSONObject()
            .put("fd", pfd.detachFd())
            .put("size", file.length())
            .put("mime", file.type ?: "application/octet-stream")
            .toString()
    }

    fun create(name: String): String {
        val withoutExt = name.substringBeforeLast('.', name)
        val extension = name.substringAfterLast('.', "")
        val unique = runBlocking { FileUtils.generateUniqueFileName(base, withoutExt, extension) }
        // Octet-stream keeps providers from appending their own extension (same as the Ktor code).
        val doc = base.createFile("application/octet-stream", unique)
            ?: return errorJson("Failed to create file.")
        val pfd = resolver.openFileDescriptor(doc.uri, "w")
            ?: return errorJson("Cannot open output stream for ${doc.uri}")
        return JSONObject()
            .put("fd", pfd.detachFd())
            .put("name", doc.name ?: unique)
            .toString()
    }

    fun delete(name: String): String {
        val file = base.findFile(name) ?: return "File not found: $name"
        return if (file.delete()) "" else "Failed to delete file: $name"
    }

    /** Blocks this Rust worker thread until the user answers the notification, like Ktor did. */
    fun approveIp(ip: String): Boolean = runBlocking { service.requestIpApprovalFromClient(ip) }

    fun onFileChanged() = service.notifyFilePushed()

    private fun errorJson(message: String) = JSONObject().put("error", message).toString()
}
