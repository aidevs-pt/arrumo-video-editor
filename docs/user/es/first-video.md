# Crea y exporta tu primer vídeo

[English](../en/first-video.md) · [Português](../pt/first-video.md)

## Configura un espacio de trabajo

Crea un espacio de trabajo y selecciona el agente de edición disponible en tu equipo. Si vas a generar subtítulos a partir del audio, configura un proveedor de transcripción que devuelva marcas de tiempo. Cada proveedor necesita sus propias credenciales. Cada campo de clave tiene **Dónde encontrarla**, que abre la página del proveedor para crearla. En el primer inicio, el paso del espacio de trabajo también pregunta dónde puede conocerte el agente: añade los enlaces de tu canal o tu web y, cuando el espacio exista, **Contexto** se abre con ellos y el agente redacta tu contexto y sugiere perfiles para que los marques.

Cada espacio de trabajo empieza con su propio icono y color. Para cambiarlos, o para mostrar una imagen como el avatar de tu canal, abre **Ajustes del espacio › Aspecto**. Solo cambia cómo se ve el espacio en Arrumo; los vídeos toman sus colores de **Marca**.

En **Ajustes del espacio › Marca**, pon nombre a cada color de marca y usa su marca de verificación para elegir si los vídeos lo usan; los colores sin ella siguen en la paleta, pero no se usan al generar.

Abre **Salidas** y comprueba qué formatos están activados. Para que la entrega se verifique, cada formato activado necesita su propio vídeo MP4 final y su archivo SRT.

## Guarda un borrador y añádelo a la cola

1. Abre **Vídeos** y entra en **Borradores**. Añade grabaciones a un borrador o elige **Empezar desde cero** y describe lo que quieres comunicar con el vídeo.
2. Pon un título al borrador e incluye las indicaciones que afecten al montaje. Guárdalo para poder retomarlo antes de iniciar la producción.
3. Elige **Generar** en el borrador. En las opciones, selecciona los proveedores y decide si quieres usar el **Piloto automático**. El piloto automático avanza por las etapas de revisión. La producción aún puede quedar a la espera de un renderizado o de archivos.
4. Activa **Generar automáticamente los subtítulos que falten** si quieres que Arrumo envíe el audio de cada formato renderizado sin SRT a un proveedor seleccionado para la ejecución. Esta opción viene desactivada.
5. Selecciona **Generar este vídeo**. Abre la pestaña **En producción** para seguir el proceso.

Mientras Arrumo está en segundo plano, los vídeos terminados, fallidos o pendientes de revisión también aparecen en las notificaciones de tu equipo. Desactívalas en **Ajustes › General › Notificaciones**.

## Revisa y termina la entrega

En una ejecución guiada, abre **Revisar proceso** cuando haya una decisión pendiente. Comprueba el guion, los recursos o la vista previa disponibles. Prepara tus decisiones y envíalas cuando hayas terminado. Si necesitas una revisión, describe el cambio en las notas de la etapa antes de enviarla.

En **Recursos y animaciones**, el agente también diseña animaciones para el vídeo, como un rótulo de título, rótulos inferiores o una tarjeta final, y Arrumo las renderiza para que las revises. Un vídeo empezado desde cero no tiene grabaciones, así que su vista previa es un guion gráfico de escenas animadas creado a partir del guion y las animaciones aprobadas. Sus subtítulos salen del texto en pantalla. Si hay un proveedor de texto a voz configurado, como ElevenLabs, el guion gráfico se narra y sus subtítulos siguen la narración. Decide cada animación con **Aprobar**, **Pedir cambios** (con una nota) o **Rechazar**, y marca **Guardar en la biblioteca al aprobar** en las que quieras conservar. Si apruebas todas y no hay nota general, el paso se aprueba; si no, vuelve al agente una revisión con cada decisión. También puedes decidir sobre todo el paso.

Los formatos que piden cortes verticales reciben vídeos verticales cortos sacados del vídeo aprobado, cada uno con sus miniaturas, listos para revisar con los títulos y las descripciones.

Para ver qué se pidió al agente y qué mostró, abre **Registros del agente** junto a la actividad del proceso. Si un paso falla, ahí verás su salida de error.

En la etapa de renderizado final, usa **Generar vídeo final** cuando esté disponible. En **Verificar la entrega**, usa **Generar subtítulos** o **Importar SRT** para cada formato que aún no tenga subtítulos. El SRT importado debe estar codificado en UTF-8 y sus marcas de tiempo deben quedar dentro de la duración del vídeo de ese formato.

Si la transcripción automática falla o se interrumpe, puedes volver a intentarlo con **Generar subtítulos** o importar un archivo. Arrumo evita reenviar automáticamente una petición interrumpida porque el proveedor podría haberla procesado ya. La generación conserva los subtítulos existentes.

La aplicación de escritorio verifica la entrega cuando todos los formatos activados tienen sus archivos MP4 y SRT. Si el proceso sigue en cola durante la verificación, comprueba si falta algún archivo en otro formato.

## Abre y exporta el resultado

Ve a **Vídeos** → **Terminados** y abre el vídeo terminado. Selecciona una salida para previsualizarla o abre **Texto de los subtítulos** para consultar sus marcas de tiempo. Usa **Exportar ZIP** para guardar los archivos verificados en un ZIP nuevo. Debajo del vídeo, **Cómo se hizo** muestra cada paso con los comentarios que dejaste, donde puedes comentar para que el agente lo lea cuando este vídeo se vuelva a generar, además de las animaciones usadas y el registro de actividad. **Ajustes usados** ofrece repetirlo con los ajustes capturados o los actuales y permite renombrar el vídeo.

## Copia de seguridad y restauración

**Ajustes › Importar y exportar** guarda todo (ajustes, espacios de trabajo, perfiles, recursos, animaciones, vídeos terminados y complementos instalados) en un `.zip`. Las claves de API no se incluyen: se quedan en el almacén de credenciales de este equipo, así que vuelve a añadirlas en un equipo nuevo.

Para restaurar, elige una copia en el mismo sitio. Arrumo comprueba primero cada archivo con las sumas de la copia, después muestra lo que contiene (espacios de trabajo, complementos, tamaño) y pregunta antes de reemplazar nada; **Exportar los míos antes** guarda tus datos actuales antes de decidir. Se reinicia para restaurarla y guarda los datos reemplazados en una carpeta llamada `workspaces.sqlite.before-import-…` junto a tus datos, por si los necesitas.

## Datos y privacidad

Arrumo cuenta qué funciones se usan e informa de los errores, para saber qué mejorar. Ambas opciones están activadas por defecto y se pueden desactivar en **Ajustes › General › Datos y privacidad**. Van ligadas a un identificador aleatorio de esta instalación, nunca a ti, y nunca incluyen tus vídeos, archivos, textos ni claves. Las compilaciones que haces tú no envían nada.

## Créditos de los proveedores

Ajustes › Servicios y claves muestra lo que le queda a cada clave cuando su complemento puede saberlo. Con un 20% restante aparece un aviso amarillo en la parte superior, y con un 10% uno rojo; si lo cierras no vuelve hasta el siguiente nivel. La página del borrador repite el aviso encima de **Generar este vídeo**, porque un proveedor agotado detiene el vídeo en ese paso.

## Complementos nuevos

Cuando llegan complementos, desde un ZIP, el registro o una actualización de Arrumo, Arrumo los presenta en un recorrido breve, una página por complemento. Puedes saltarlo cuando quieras; **Ajustes › Complementos › Ver el recorrido de complementos** vuelve a mostrar la presentación de cada uno.
