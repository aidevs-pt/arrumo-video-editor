# Crea y exporta tu primer vídeo

[English](../en/first-video.md) · [Português](../pt/first-video.md)

## Configura un espacio de trabajo

Crea un espacio de trabajo y selecciona el agente de edición disponible en tu equipo. Si vas a generar subtítulos a partir del audio, configura un proveedor de transcripción que devuelva marcas de tiempo. Cada proveedor necesita sus propias credenciales.

Abre **Salidas** y comprueba qué formatos están activados. Para que la entrega se verifique, cada formato activado necesita su propio vídeo MP4 final y su archivo SRT.

## Guarda un borrador y añádelo a la cola

1. Abre **Vídeos** y entra en **Borradores**. Añade grabaciones a un borrador o elige **Empezar desde cero** y describe lo que quieres comunicar con el vídeo.
2. Pon un título al borrador e incluye las indicaciones que afecten al montaje. Guárdalo para poder retomarlo antes de iniciar la producción.
3. Elige **Generar** en el borrador. En las opciones, selecciona los proveedores y decide si quieres usar el **Piloto automático**. El piloto automático avanza por las etapas de revisión. La producción aún puede quedar a la espera de un renderizado o de archivos.
4. Activa **Generar automáticamente los subtítulos que falten** si quieres que Arrumo envíe el audio de cada formato renderizado sin SRT a un proveedor seleccionado para la ejecución. Esta opción viene desactivada.
5. Selecciona **Generar este vídeo**. Abre la pestaña **En producción** para seguir el proceso.

## Revisa y termina la entrega

En una ejecución guiada, abre **Revisar proceso** cuando haya una decisión pendiente. Comprueba el guion, los recursos o la vista previa disponibles. Prepara tus decisiones y envíalas cuando hayas terminado. Si necesitas una revisión, describe el cambio en las notas de la etapa antes de enviarla.

En **Recursos y animaciones**, el agente también diseña animaciones para el vídeo, como un rótulo de título, rótulos inferiores o una tarjeta final, y Arrumo las renderiza para que las revises. Un vídeo empezado desde cero no tiene grabaciones, así que su vista previa es un guion gráfico de escenas animadas creado a partir del guion y las animaciones aprobadas. Sus subtítulos salen del texto en pantalla. Si hay un proveedor de texto a voz configurado, como ElevenLabs, el guion gráfico se narra y sus subtítulos siguen la narración.

Los formatos que piden cortes verticales reciben vídeos verticales cortos sacados del vídeo aprobado, cada uno con sus miniaturas, listos para revisar con los títulos y las descripciones.

Para ver qué se pidió al agente y qué mostró, abre **Registros del agente** junto a la actividad del proceso. Si un paso falla, ahí verás su salida de error.

En la etapa de renderizado final, usa **Generar vídeo final** cuando esté disponible. En **Verificar la entrega**, usa **Generar subtítulos** o **Importar SRT** para cada formato que aún no tenga subtítulos. El SRT importado debe estar codificado en UTF-8 y sus marcas de tiempo deben quedar dentro de la duración del vídeo de ese formato.

Si la transcripción automática falla o se interrumpe, puedes volver a intentarlo con **Generar subtítulos** o importar un archivo. Arrumo evita reenviar automáticamente una petición interrumpida porque el proveedor podría haberla procesado ya. La generación conserva los subtítulos existentes.

La aplicación de escritorio verifica la entrega cuando todos los formatos activados tienen sus archivos MP4 y SRT. Si el proceso sigue en cola durante la verificación, comprueba si falta algún archivo en otro formato.

## Abre y exporta el resultado

Ve a **Vídeos** → **Terminados** y abre el vídeo terminado. Selecciona una salida para previsualizarla o abre **Texto de los subtítulos** para consultar sus marcas de tiempo. Usa **Exportar ZIP** para guardar los archivos verificados en un ZIP nuevo.

## Copia de seguridad y restauración

**Ajustes › Importar y exportar** guarda todo (ajustes, espacios de trabajo, perfiles, recursos, animaciones, vídeos terminados y plugins instalados) en un `.zip`. Las claves de API no se incluyen: se quedan en el almacén de credenciales de este equipo, así que vuelve a añadirlas en un equipo nuevo.

Para restaurar, elige una copia en el mismo sitio. Arrumo comprueba primero cada archivo con las sumas de la copia, después muestra lo que contiene y pregunta antes de reemplazar nada. Se reinicia para restaurarla y guarda los datos reemplazados en una carpeta llamada `workspaces.sqlite.before-import-…` junto a tus datos, por si los necesitas.

## Datos y privacidad

Arrumo cuenta qué funciones se usan e informa de los errores, para saber qué mejorar. Ambas opciones están activadas por defecto y se pueden desactivar en **Ajustes › General › Datos y privacidad**. Van ligadas a un identificador aleatorio de esta instalación, nunca a ti, y nunca incluyen tus vídeos, archivos, textos ni claves. Las compilaciones que haces tú no envían nada.
