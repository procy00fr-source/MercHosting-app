// Servidor de PRUEBA para los tests del supervisor. No es Minecraft.
// Imita el formato de log de Log4j del servidor dedicado, lee comandos por stdin,
// abre de verdad el puerto de server.properties y responde a "stop" como Minecraft.
// Modos (primer argumento distinto de "nogui", vía -Dfake.mode=...):
//   normal  -> carga, "Done", espera comandos
//   crash   -> sale con código 1 durante la carga
//   hang    -> nunca imprime "Done"
//   nostop  -> ignora "stop"
// Si existe la carpeta plugins/LuckPerms, imita lo justo de LuckPerms (comando "luckperms"):
// export <nombre> (JSON comprimido con gzip, como el real), creategroup, deletegroup,
// group|user <x> permission set|unset y parent add|remove. Sin contextos.
// Como Minecraft, si su consola (stdin) se cierra, el servidor SIGUE funcionando.
// Seguridad para pruebas: se cierra solo tras -Dfake.maxSeconds (por defecto 120 s)
// para no dejar procesos huérfanos si una prueba falla.
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
    // ---- Mini LuckPerms de prueba ----
    static final Path LP_DIR = Paths.get("plugins", "LuckPerms");
    // nombre (grupo) o uuid (usuario) -> nodo -> valor
    static final Map<String, Map<String, Boolean>> groups = new TreeMap<>();
    static final Map<String, Map<String, Boolean>> users = new TreeMap<>();
    static final Map<String, String> usernames = new HashMap<>();
    static {
        groups.put("default", new TreeMap<>());
        Map<String, Boolean> admin = new TreeMap<>();
        admin.put("essentials.fly", true);
        admin.put("weight.10", true);
        admin.put("prefix.10.&c[Admin] ", true);
        admin.put("group.default", true);
        groups.put("admin", admin);
        Map<String, Boolean> u = new TreeMap<>();
        u.put("group.admin", true);
        u.put("essentials.home", false);
        users.put("069a79f4-44e9-4726-a5be-fca90e38aaf5", u);
        usernames.put("069a79f4-44e9-4726-a5be-fca90e38aaf5", "Notch");
    }
    static String q(String s) { return "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"") + "\""; }
    static String nodes(Map<String, Boolean> m) {
        StringBuilder b = new StringBuilder("[");
        for (Map.Entry<String, Boolean> e : m.entrySet()) {
            if (b.length() > 1) b.append(',');
            String k = e.getKey();
            String type = k.startsWith("group.") ? "inheritance" : k.startsWith("weight.") ? "weight"
                    : k.startsWith("prefix.") ? "prefix" : "permission";
            b.append("{\"type\":").append(q(type)).append(",\"key\":").append(q(k))
             .append(",\"value\":").append(e.getValue()).append('}');
        }
        return b.append(']').toString();
    }
    static void fakeLuckPerms(String[] a) throws Exception {
        if (a[0].equals("export") && a.length == 2) {
            Path f = LP_DIR.resolve(a[1] + ".json.gz");
            if (Files.exists(f)) { log("INFO", "[LP] Error: the file already exists"); return; }
            StringBuilder j = new StringBuilder("{\"metadata\":{\"generatedBy\":\"Console\",\"generatedAt\":\"2026-10-30 12:00:00 UTC\"},\"groups\":{");
            boolean first = true;
            for (Map.Entry<String, Map<String, Boolean>> g : groups.entrySet()) {
                if (!first) j.append(','); first = false;
                j.append(q(g.getKey())).append(":{\"nodes\":").append(nodes(g.getValue())).append('}');
            }
            j.append("},\"tracks\":{},\"users\":{");
            first = true;
            for (Map.Entry<String, Map<String, Boolean>> u : users.entrySet()) {
                if (!first) j.append(','); first = false;
                j.append(q(u.getKey())).append(":{\"username\":").append(q(usernames.get(u.getKey())))
                 .append(",\"nodes\":").append(nodes(u.getValue())).append('}');
            }
            j.append("}}");
            Files.createFile(f);
            Thread.sleep(150);
            try (OutputStream o = new java.util.zip.GZIPOutputStream(Files.newOutputStream(f))) {
                o.write(j.toString().getBytes(StandardCharsets.UTF_8));
            }
            log("INFO", "[LP] Successfully exported to " + f.toAbsolutePath());
        } else if (a[0].equals("creategroup") && a.length == 2) {
            groups.putIfAbsent(a[1].toLowerCase(), new TreeMap<>());
            log("INFO", "[LP] " + a[1] + " was successfully created.");
        } else if (a[0].equals("deletegroup") && a.length == 2) {
            groups.remove(a[1].toLowerCase());
            log("INFO", "[LP] Successfully deleted group " + a[1] + ".");
        } else if ((a[0].equals("group") || a[0].equals("user")) && a.length >= 5) {
            Map<String, Map<String, Boolean>> where = a[0].equals("group") ? groups : users;
            Map<String, Boolean> n = where.get(a[1].toLowerCase());
            if (n == null) { log("INFO", "[LP] " + a[1] + " does not exist."); return; }
            String op = a[2] + " " + a[3];
            if (op.equals("permission set")) n.put(a[4], a.length < 6 || !a[5].equals("false"));
            else if (op.equals("permission unset")) n.remove(a[4]);
            else if (op.equals("parent add") && groups.containsKey(a[4].toLowerCase())) n.put("group." + a[4].toLowerCase(), true);
            else if (op.equals("parent remove")) n.remove("group." + a[4].toLowerCase());
            else { log("INFO", "[LP] Command not understood."); return; }
            log("INFO", "[LP] Done: " + String.join(" ", a));
        } else {
            log("INFO", "[LP] Command not understood.");
        }
    }

    public static void main(String[] args) throws Exception {
        out = new PrintStream(new FileOutputStream(FileDescriptor.out), true, "UTF-8");
        String mode = System.getProperty("fake.mode", "normal");
        long maxSeconds = Long.parseLong(System.getProperty("fake.maxSeconds", "120"));
        Thread watchdog = new Thread(() -> {
            try { Thread.sleep(maxSeconds * 1000L); } catch (InterruptedException e) {}
            Runtime.getRuntime().halt(3);
        });
        watchdog.setDaemon(true);
        watchdog.start();
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
            } else if (line.startsWith("fakejoin ")) {
                log("INFO", line.substring(9) + " joined the game");
            } else if (line.startsWith("fakeleave ")) {
                log("INFO", line.substring(10) + " left the game");
            } else if (line.equals("tps")) {
                log("INFO", "\u00a76TPS from last 1m, 5m, 15m: \u00a7a*20.0, \u00a7a19.9, \u00a7a19.95");
            } else if (line.equals("fakecrash")) {
                System.err.println("java.lang.OutOfMemoryError: prueba de caída");
                System.exit(1);
            } else if (line.equals("save-off")) {
                log("INFO", "Automatic saving is now disabled");
            } else if (line.equals("save-all flush")) {
                log("INFO", "Saving the game (this may take a moment!)");
                Thread.sleep(100);
                log("INFO", "Saved the game");
            } else if (line.equals("save-on")) {
                log("INFO", "Automatic saving is now enabled");
            } else if (line.startsWith("luckperms ") && Files.isDirectory(LP_DIR)) {
                fakeLuckPerms(line.substring(10).trim().split(" +"));
            } else if (line.startsWith("gamerule ")) {
                // Imita la respuesta de /gamerule (consulta o cambio).
                String[] gr = line.split(" ");
                if (gr.length == 2) log("INFO", "Gamerule " + gr[1] + " is currently set to: " + (gr[1].contains("ick") ? "3" : "false"));
                else log("INFO", "Gamerule " + gr[1] + " is now set to: " + gr[2]);
            } else {
                log("INFO", "Comando recibido: " + line);
            }
        }
        // stdin cerrado (por ejemplo, el supervisor se cayó): el servidor sigue vivo.
        // Sin consola, solo se puede detener desde "dentro": aquí, creando el archivo stop.flag.
        while (!Files.exists(Paths.get("stop.flag"))) { Thread.sleep(200); }
        ss.close();
        System.exit(0);
    }
}
