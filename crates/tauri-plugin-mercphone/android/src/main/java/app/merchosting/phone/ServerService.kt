package app.merchosting.phone

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import android.os.Process
import androidx.core.app.NotificationCompat
import java.io.File
import java.io.FileOutputStream

/**
 * Servicio en primer plano (proceso ":server") que corre UN servidor de Minecraft Java.
 * Una JVM solo se puede crear una vez por proceso: cuando el servidor termina, el proceso se
 * cierra entero y la siguiente vez Android crea uno nuevo.
 */
class ServerService : Service() {
    companion object {
        const val ACTION_START = "app.merchosting.phone.START"
        const val ACTION_STOP = "app.merchosting.phone.STOP"
        const val ACTION_KILL = "app.merchosting.phone.KILL"
        const val EXTRA_ID = "id"
        const val EXTRA_NAME = "name"
        const val EXTRA_SPEC = "spec"
        const val EXTRA_FIFO = "fifo"
        private const val CHANNEL = "merchosting_servidor"
        private const val NOTIFICATION_ID = 25565

        @Volatile
        var running = false
            private set
    }

    private var thread: Thread? = null
    private var fifo: String? = null
    private var wake: PowerManager.WakeLock? = null
    private var wifi: WifiManager.WifiLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                // Apagado limpio: el servidor guarda el mundo y la JVM cierra este proceso.
                sendStop()
                return START_NOT_STICKY
            }
            ACTION_KILL -> {
                Process.killProcess(Process.myPid())
                return START_NOT_STICKY
            }
        }
        if (thread != null || intent == null) return START_NOT_STICKY
        val name = intent.getStringExtra(EXTRA_NAME) ?: "Servidor"
        val spec = intent.getStringExtra(EXTRA_SPEC) ?: return START_NOT_STICKY
        fifo = intent.getStringExtra(EXTRA_FIFO)
        startInForeground(name)
        acquireLocks()
        running = true
        val t = Thread({
            var code = -1
            try {
                MercNative.load()
                code = MercNative.launch(spec)
            } catch (e: Throwable) {
                android.util.Log.e("MercHosting", "no se pudo lanzar el servidor", e)
            }
            android.util.Log.i("MercHosting", "el servidor terminó ($code)")
            finish()
        }, "merc-servidor")
        thread = t
        t.start()
        // Si Android cierra el proceso, no se vuelve a encender solo: lo decide la persona.
        return START_NOT_STICKY
    }

    private fun startInForeground(name: String) {
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= 26) {
            nm.createNotificationChannel(
                NotificationChannel(CHANNEL, "Servidor encendido", NotificationManager.IMPORTANCE_LOW)
            )
        }
        val flags = PendingIntent.FLAG_UPDATE_CURRENT or
            (if (Build.VERSION.SDK_INT >= 23) PendingIntent.FLAG_IMMUTABLE else 0)
        val stop = PendingIntent.getService(
            this, 1, Intent(this, ServerService::class.java).setAction(ACTION_STOP), flags
        )
        val open = packageManager.getLaunchIntentForPackage(packageName)?.let {
            PendingIntent.getActivity(this, 2, it, flags)
        }
        val n: Notification = NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(applicationInfo.icon)
            .setContentTitle("$name está encendido en este celular")
            .setContentText("Gasta batería y puede calentar el celular. Toca Detener para apagarlo y guardar el mundo.")
            .setOngoing(true)
            .setContentIntent(open)
            .addAction(0, "Detener", stop)
            .build()
        // Android 14+ exige declarar el tipo de servicio en primer plano; desde Android 10 existe
        // la versión con tipo. Antes, la llamada clásica.
        when {
            Build.VERSION.SDK_INT >= 34 ->
                startForeground(NOTIFICATION_ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
            Build.VERSION.SDK_INT >= 29 -> startForeground(NOTIFICATION_ID, n, 0)
            else -> startForeground(NOTIFICATION_ID, n)
        }
    }

    @Suppress("DEPRECATION")
    private fun acquireLocks() {
        val pm = getSystemService(Context.POWER_SERVICE) as PowerManager
        wake = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "MercHosting:servidor").apply {
            setReferenceCounted(false)
            acquire()
        }
        val wm = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        wifi = wm.createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "MercHosting:servidor").apply {
            setReferenceCounted(false)
            acquire()
        }
    }

    private fun sendStop() {
        val f = fifo ?: return
        try {
            // El servidor tiene la tubería abierta: escribir no se bloquea.
            FileOutputStream(File(f), true).use { it.write("stop\n".toByteArray()) }
        } catch (e: Exception) {
            android.util.Log.w("MercHosting", "no se pudo mandar stop", e)
        }
    }

    private fun finish() {
        running = false
        try { wake?.release() } catch (_: Exception) {}
        try { wifi?.release() } catch (_: Exception) {}
        if (Build.VERSION.SDK_INT >= 24) {
            stopForeground(Service.STOP_FOREGROUND_REMOVE)
        } else {
            @Suppress("DEPRECATION")
            stopForeground(true)
        }
        stopSelf()
        // La JVM no se puede crear dos veces en el mismo proceso: se cierra entero.
        Process.killProcess(Process.myPid())
    }
}
