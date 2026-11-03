#!/bin/sh
# Recompila merc-agent.jar desde src/ (necesita un JDK 17 o más nuevo). El .jar va en el
# repositorio para no exigir un JDK al compilar MercHosting; una prueba comprueba que el .jar
# trae la clase y el Premain-Class correctos.
set -e
cd "$(dirname "$0")"
rm -rf build && mkdir build
javac --release 17 -d build src/app/merchosting/agent/MercAgent.java
# Fechas fijas para que el .jar sea igual en cada compilación.
find build -exec touch -d 1980-01-01T00:00:00Z {} +
jar --create --file merc-agent.jar --manifest MANIFEST.MF --date=1980-01-01T00:00:02Z -C build .
rm -rf build
