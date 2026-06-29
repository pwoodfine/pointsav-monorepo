# service-ingress

Proxy inverso de terminación TLS para `os-console` (Fase A mTLS). Expone los
servicios locales a través de HTTPS/TLS 1.3 para que `os-console` pueda conectarse
desde una computadora portátil remota sin requerir el puerto 2222.

## Función

Escucha en `0.0.0.0:8443`. Al iniciar por primera vez, genera automáticamente una CA
autofirmada y un certificado de servidor bajo `~/.config/service-ingress/` e imprime la
huella SHA-256. `os-console` fija esta huella (auto-fijación TOFU en
`~/.config/os-console/server-hostkey`); las conexiones posteriores verifican contra el
valor fijado, rechazando cualquier MITM.

Enrutamiento de rutas (todos los destinos son solo localhost):

| Prefijo de ruta | Destino |
|---|---|
| `/v1/proof/*` | `http://127.0.0.1:9092` (service-content) |
| `/v1/content/*` | `http://127.0.0.1:9092` (service-content) |
| `/v1/search/*` | `http://127.0.0.1:9092` (service-content) |
| `/doorman/*` | `http://127.0.0.1:9080` (Doorman) |
| `/health` | `200 OK {"status":"ok"}` |

## Configuración

Archivo de configuración opcional en `~/.config/service-ingress/config.toml`:

```toml
[listen]
port = 8443

[upstream]
content = "http://127.0.0.1:9092"
doorman = "http://127.0.0.1:9080"
```

## Compilación y ejecución

```bash
cargo build --release -p service-ingress
./target/release/service-ingress
```

## Rol en la arquitectura de os-console

`service-ingress` es la capa de transporte de la Fase A. Permite que `os-console` se
conecte a un Archivo Totebox remoto a través de redes corporativas o de hotel (tráfico
saliente en 443/8443; sin reglas de firewall entrantes en la computadora portátil). El
puerto 2222 de GCE permanece cerrado. El enlace mTLS (ceremonia de emparejamiento MBA)
es la puerta en la capa de aplicación; `service-ingress` es el canal.

Fase B planificada: subyacente WireGuard PPN para entornos NAT/CGNAT.

## Licencia

AGPL-3.0-or-later. Consulte `LICENSE` en la raíz del repositorio.
