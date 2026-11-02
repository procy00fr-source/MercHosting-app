package app.merchosting.phone

/** Puente JNI hacia la biblioteca de la app (`libmerchosting_app_lib.so`). */
object MercNative {
    @Volatile
    private var loaded = false

    fun load() {
        if (!loaded) {
            System.loadLibrary("merchosting_app_lib")
            loaded = true
        }
    }

    /**
     * Carga el JRE con dlopen y llama a JLI_Launch con la especificación (JSON de
     * `merc_phone::jli::LaunchSpec`). Bloquea mientras el servidor corre. Si el servidor llama a
     * System.exit, este proceso termina ahí mismo.
     */
    @JvmStatic
    external fun launch(spec: String): Int
}
