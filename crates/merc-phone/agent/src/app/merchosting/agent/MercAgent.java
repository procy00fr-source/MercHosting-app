// MercHosting — ayudante de diagnóstico para los servidores del celular (GPL-3.0-or-later).
//
// Se carga con -javaagent:merc-agent.jar=<archivo>. Cada segundo mira si existe <archivo>; si
// existe, lo borra y escribe en la consola del servidor (la salida 1 del proceso) qué está
// haciendo cada hilo de la JVM. En Android no hay jstack ni jcmd, y esto es lo que permite ver
// dónde se queda trabado un servidor. No abre puertos, no lee nada más y no cambia nada.
package app.merchosting.agent;

import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.PrintStream;
import java.lang.instrument.Instrumentation;
import java.lang.management.ManagementFactory;
import java.lang.management.ThreadInfo;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

public final class MercAgent {
    private MercAgent() {}

    public static void premain(String args, Instrumentation inst) {
        if (args == null || args.isEmpty()) return;
        final Path trigger = Paths.get(args);
        // Directo a la salida 1 (la consola), sin pasar por el System.out que reemplaza Paper.
        final PrintStream out = new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
        Thread t = new Thread(() -> {
            while (true) {
                try {
                    Thread.sleep(1000);
                    if (Files.deleteIfExists(trigger)) dump(out);
                } catch (InterruptedException e) {
                    return;
                } catch (Throwable e) {
                    out.println("[MercHosting] No se pudo hacer el volcado de hilos: " + e);
                }
            }
        }, "merc-diagnostico");
        t.setDaemon(true);
        t.start();
    }

    static void dump(PrintStream out) {
        ThreadInfo[] all = ManagementFactory.getThreadMXBean().dumpAllThreads(true, true);
        StringBuilder sb = new StringBuilder();
        sb.append("[MercHosting] ===== Volcado de hilos (").append(all.length).append(") =====\n");
        for (ThreadInfo ti : all) {
            if (ti == null) continue;
            sb.append("[MercHosting] \"").append(ti.getThreadName()).append("\" ").append(ti.getThreadState());
            if (ti.getLockName() != null) sb.append(" en ").append(ti.getLockName());
            if (ti.getLockOwnerName() != null) sb.append(" (lo tiene \"").append(ti.getLockOwnerName()).append("\")");
            sb.append('\n');
            StackTraceElement[] st = ti.getStackTrace();
            int n = Math.min(st.length, 25);
            for (int i = 0; i < n; i++) sb.append("[MercHosting]     at ").append(st[i]).append('\n');
            if (st.length > n) sb.append("[MercHosting]     … ").append(st.length - n).append(" más\n");
        }
        sb.append("[MercHosting] ===== Fin del volcado =====");
        out.println(sb);
    }
}
