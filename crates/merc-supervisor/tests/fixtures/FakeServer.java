// Servidor de PRUEBA para los tests del supervisor. No es Minecraft.
// Imita el formato de log de Log4j del servidor dedicado, lee comandos por stdin,
// abre de verdad el puerto de server.properties y responde a "stop" como Minecraft.
// Modos (primer argumento distinto de "nogui", vía -Dfake.mode=...):
//   normal  -> carga, "Done", espera comandos
//   crash   -> sale con código 1 durante la carga
//   hang    -> nunca imprime "Done"
//   nostop  -> ignora "stop"
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;

public class FakeServer {
    static PrintStream out;
    static void log(String level, String msg) {
        out.println("[12:00:00] [Server thread/" + level + "]: " + msg);
        out.flush();
    }
    public static void main(String[] args) throws Exception {
        out = new PrintStream(new FileOutputStream(FileDescriptor.out), true, "UTF-8");
        String mode = System.getProperty("fake.mode", "normal");
        Properties p = new Properties();
        try (Reader r = Files.newBufferedReader(Paths.get("server.properties"))) { p.load(r); } catch (Exception e) {}
        int port = Integer.parseInt(p.getProperty("server-port", "25565"));
        log("INFO", "Starting minecraft server version 1.21.1");
        log("INFO", "Loading properties");
        Thread.sleep(300);
        if (mode.equals("crash")) {
            System.err.println("java.lang.RuntimeException: mod defectuoso (prueba)");
            System.exit(1);
        }
        ServerSocket ss;
        try { ss = new ServerSocket(port); }
        catch (IOException e) { log("WARN", "**** FAILED TO BIND TO PORT!"); System.exit(1); return; }
        log("INFO", "Preparing level \"world\"");
        log("INFO", "Mundo con tildes: áéíóú ñ");
        if (mode.equals("hang")) { Thread.sleep(600000); }
        log("INFO", "Done (1.234s)! For help, type \"help\"");
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        String line;
        while ((line = in.readLine()) != null) {
            if (line.equals("stop") && !mode.equals("nostop")) {
                log("INFO", "Stopping server");
                log("INFO", "Saving worlds");
                Thread.sleep(300);
                ss.close();
                System.exit(0);
            } else {
                log("INFO", "Comando recibido: " + line);
            }
        }
    }
}
