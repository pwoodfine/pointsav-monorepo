<div align="center">

<img src="https://raw.githubusercontent.com/pointsav/pointsav-media-assets/main/ASSET-SIGNET-MASTER.svg" width="72" alt="PointSav Digital Systems">

# PointSav Digital Systems
### *Infraestructura de Registros Verificables para Instituciones que Poseen sus Activos*

[![Licencia: Apache 2.0](https://img.shields.io/badge/Licencia-Apache_2.0-blue.svg?style=flat-square)](https://opensource.org/licenses/Apache-2.0)
[![Cumplimiento: WORM](https://img.shields.io/badge/Cumplimiento-WORM_Listo-22863a.svg?style=flat-square)](#)
[![Fundación: seL4 Verificado](https://img.shields.io/badge/Fundaci%C3%B3n-seL4_Verificado-6f42c1.svg?style=flat-square)](#la-fundación-de-sistemas-verificables)

<br/>

**[→ Wiki de Documentación](https://github.com/pointsav/content-wiki-documentation)** &nbsp;·&nbsp; **[→ Sistema de Diseño](https://github.com/pointsav/pointsav-design-system)** &nbsp;·&nbsp; **[→ Despliegue Operativo](https://github.com/woodfine/woodfine-fleet-deployment)** &nbsp;·&nbsp; **[→ pointsav.com](https://pointsav.com)**

</div>

<br/>

> [!NOTE]
> El rol de supervisión previsto para la Sovereign Data Foundation aún no ha sido ejecutado formalmente. Este repositorio no contiene cargas útiles de red propietarias activas.

---

## El Problema

Los registros de su organización viven en software que usted no posee. El proveedor controla el acceso. Si modifica sus condiciones, aumenta sus precios o cierra operaciones, sus registros quedan retenidos bajo sus condiciones. Este es el acuerdo predeterminado para toda institución que almacena libros financieros, registros de propiedades y expedientes de personal en una plataforma comercial en la nube. Ha sido siempre el acuerdo. La mayoría de las instituciones lo han aceptado.

PointSav fue construido para las que no lo harán.

Desarrollamos sistemas operativos — no aplicaciones — donde cada archivo es un entorno autónomo y verificado, vinculado a un activo jurídico específico. Un inmueble. Una empresa. Una persona. El archivo pertenece a la institución. No a nosotros. No a un proveedor de nube. Cuando el activo cambia de manos, el historial completo se transfiere con él.

---

## Cinco Diferencias Estructurales

### I. Propiedad a Nivel de Activo

Cada archivo está vinculado a un identificador jurídico específico — un título de propiedad, un número de registro empresarial, un número de pasaporte. El archivo pertenece exclusivamente a ese activo. Cuando se vende un inmueble, el archivo se transfiere con el título. Ningún proveedor de nube ofrece este concepto porque es incompatible con su modelo multiusuario.

Cada instancia de ToteboxOS es un unikernel independiente — un sistema operativo mínimo que contiene únicamente el núcleo y los servicios necesarios para ese activo específico. Dos archivos no comparten nunca el mismo núcleo.

### II. Seguridad Formalmente Verificada

La seguridad de la mayoría de las infraestructuras es probada — los ingenieros intentan vulnerarla y corrigen lo que encuentran. Las pruebas demuestran la presencia de algunos fallos. No pueden demostrar la ausencia de todos ellos.

La base de seguridad de PointSav utiliza el micronúcleo seL4, cuyas propiedades de seguridad han sido verificadas formalmente mediante pruebas matemáticas comprobadas por máquina — la misma técnica utilizada en software de aviación y dispositivos médicos. Este es un resultado revisado por pares, publicado en el Simposio ACM sobre Principios de Sistemas Operativos (SOSP 2009). seL4 es el único sistema operativo de propósito general con esta propiedad.

### III. Permanencia de Archivos Planos

PointSav almacena todos los registros como archivos planos inertes — Markdown, YAML, CSV, JSON. Un archivo `.yaml` escrito hoy no requiere ningún software propietario para ser leído dentro de cincuenta años. Los datos sobreviven al software.

Las sumas de verificación criptográficas SHA-256 sellan cada registro en el momento de su ingreso, haciendo que cualquier alteración posterior sea detectable por cualquier auditor con una terminal estándar.

### IV. Economía de Nodo Básico

El despliegue base de ToteboxOS funciona en un nodo en la nube de bajo costo — aproximadamente $7 USD al mes. Sin hardware propietario. Sin compromiso mínimo. La escalera comercial es transparente: archivo base a precio básico; procesamiento local de IA como mejora opcional de hardware; orquestación multiarchivo como capa comercial propietaria.

### V. Sin Bloqueo de Salida de Datos

El formato de exportación definitivo para cada archivo de ToteboxOS es una Imagen de Disco de Arranque — un archivo de máquina virtual que arranca en cualquier hipervisor estándar. El historial completo del archivo puede trasladarse a una unidad USB y arrancarse en cualquier computadora compatible del mundo.

---

## La Fundación de Sistemas Verificables

El modelo de activación por niveles:

| Nivel | Nodo | Qué activa |
|---|---|---|
| Base | ~$7/mes nodo básico | ToteboxOS — archivos planos, cumplimiento WORM, sellado SHA-256, búsqueda. Cero dependencia de IA. |
| Habilitado para IA | Nodo con especificaciones mínimas para `service-slm` | Procesamiento local de IA. El protocolo portero para IA externa queda disponible. Los datos corporativos nunca salen de la red privada. |
| Orquestación | `os-orchestration` | Operaciones multiarchivo. Computación extendida para BIM, SIG y almacén de datos. La capa comercial propietaria. |

---

## El Modelo Comercial

El uso de archivo único — una instancia de ToteboxOS, una terminal ConsoleOS — es completamente gratuito y de código abierto bajo Apache 2.0.

En el momento en que necesite agregar información de múltiples archivos — por ejemplo, conectar los registros de propiedad de un inmueble con los registros de personal del equipo de gestión — necesitará OrchestrationOS, que es software propietario. PointSav no cobra por el almacenamiento privado de datos. Cobra por la capa de inteligencia que conecta los archivos entre sí.

PointSav sigue un modelo comercial de costo más margen fijo. El tiempo de desarrollo se factura al costo más un margen fijo. Los precios basados en valor añadido se rechazan explícitamente.

---

## La Prueba en Vivo

Woodfine Management Corp., empresa de gestión inmobiliaria que opera en América del Norte y Europa, está ejecutando una transformación digital completa sobre la plataforma PointSav — desplegando archivos verificables y portátiles para registros de propiedades, gobernanza corporativa y gestores operativos, con componentes selectivos activos y el stack completo en desarrollo activo.

El manifiesto de despliegue de flota de Woodfine — 201 confirmaciones y en crecimiento — está disponible en **[github.com/woodfine/woodfine-fleet-deployment](https://github.com/woodfine/woodfine-fleet-deployment)**.

---

## Contacto

**pointsav.com** &nbsp;·&nbsp; **open.source@pointsav.com** &nbsp;·&nbsp; **[github.com/pointsav](https://github.com/pointsav)**

---

*© 2026 PointSav Digital Systems™. Los componentes con licencia Apache 2.0 se rigen por los términos de dicha licencia. Los componentes propietarios están todos los derechos reservados.*

*→ English version: [README.md](./README.md)*
