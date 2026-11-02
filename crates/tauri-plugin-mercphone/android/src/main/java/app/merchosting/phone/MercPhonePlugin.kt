package app.merchosting.phone

import android.app.Activity
import android.app.ActivityManager
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.content.ContextCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class StartArgs {
    lateinit var id: String
    lateinit var name: String
    lateinit var spec: String
    lateinit var fifo: String
}

@TauriPlugin
class MercPhonePlugin(private val activity: Activity) : Plugin(activity) {

    /** Lo que el celular dice de sí mismo (nada se inventa: viene de Android). */
    @Command
    fun deviceInfo(invoke: Invoke) {
        val am = activity.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
        val mem = ActivityManager.MemoryInfo()
        am.getMemoryInfo(mem)
        val ret = JSObject()
        ret.put("totalRamMb", mem.totalMem / (1024L * 1024L))
        ret.put("abi", Build.SUPPORTED_ABIS.firstOrNull() ?: "")
        ret.put("nativeLibraryDir", activity.applicationInfo.nativeLibraryDir ?: "")
        ret.put("sdk", Build.VERSION.SDK_INT)
        invoke.resolve(ret)
    }

    @Command
    fun startServer(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(StartArgs::class.java)
            val i = Intent(activity, ServerService::class.java)
                .setAction(ServerService.ACTION_START)
                .putExtra(ServerService.EXTRA_ID, args.id)
                .putExtra(ServerService.EXTRA_NAME, args.name)
                .putExtra(ServerService.EXTRA_SPEC, args.spec)
                .putExtra(ServerService.EXTRA_FIFO, args.fifo)
            ContextCompat.startForegroundService(activity, i)
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: "No se pudo encender el servicio del servidor.")
        }
    }

    /** Cierra el proceso del servidor sin esperar (solo si no responde a «Detener»). */
    @Command
    fun forceStop(invoke: Invoke) {
        try {
            val am = activity.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
            val name = activity.packageName + ":server"
            val proc = am.runningAppProcesses?.firstOrNull { it.processName == name }
            if (proc != null) android.os.Process.killProcess(proc.pid)
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: "No se pudo cerrar el servidor.")
        }
    }

    @Command
    fun serviceRunning(invoke: Invoke) {
        val am = activity.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
        val name = activity.packageName + ":server"
        val ret = JSObject()
        ret.put("running", am.runningAppProcesses?.any { it.processName == name } == true)
        invoke.resolve(ret)
    }
}
