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

En la etapa de renderizado final, usa **Generar vídeo final** cuando esté disponible. En **Verificar la entrega**, usa **Generar subtítulos** o **Importar SRT** para cada formato que aún no tenga subtítulos. El SRT importado debe estar codificado en UTF-8 y sus marcas de tiempo deben quedar dentro de la duración del vídeo de ese formato.

Si la transcripción automática falla o se interrumpe, puedes volver a intentarlo con **Generar subtítulos** o importar un archivo. Arrumo evita reenviar automáticamente una petición interrumpida porque el proveedor podría haberla procesado ya. La generación conserva los subtítulos existentes.

La aplicación de escritorio verifica la entrega cuando todos los formatos activados tienen sus archivos MP4 y SRT. Si el proceso sigue en cola durante la verificación, comprueba si falta algún archivo en otro formato.

## Abre y exporta el resultado

Ve a **Vídeos** → **Terminados** y abre el vídeo terminado. Selecciona una salida para previsualizarla o abre **Texto de los subtítulos** para consultar sus marcas de tiempo. Usa **Exportar ZIP** para guardar los archivos verificados en un ZIP nuevo.
