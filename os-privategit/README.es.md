# OS PrivateGit

<div align="center">

[ 🇬🇧 Read this document in English ](./README.md)

</div>

### *Sistema Operativo de Distribución de Software — Instalador Completo*

**Plataforma:** x86_64-unknown-linux-gnu
**Estado:** Estructura de ingeniería (ciclo de ingeniería pendiente)

## Mandato Arquitectónico

`os-privategit` genera la imagen de sistema operativo de arranque que despliega la pila
de distribución de software de PointSav en hardware propiedad del cliente. Instala y
opera tres servicios cooperativos:

| Servicio | Puerto | Función |
|---|---|---|
| `app-privategit-marketplace` | 9202 | Tienda — catálogo de productos, verificación de pagos, emisión de licencias |
| `app-privategit-source` | 9201 | Servidor de binarios — descargas autenticadas, verificación de tokens Ed25519 |
| `tool-wallet` | — | Observador de pagos USDC en Polygon — escritor de recibos, derivación de direcciones HD |

Este es el producto distribuible principal orientado al cliente. Los operadores de pequeñas
empresas despliegan `os-privategit` para alojar de forma autónoma la pila completa de
`software.pointsav.com` en hardware bajo su control, sin enrutar código fuente ni flujos
de pago a través de un proveedor externo.

**Autoridad de Licencias:** `os-privategit` mantiene el registro de licencias Ed25519 y
la ruta del seed de la billetera. Ambos son aprovisionados por el operador en el primer
arranque y nunca abandonan la máquina anfitriona.
