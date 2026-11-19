const en = {
  title: "Assets", back: "Back to profiles", list: "Workspace assets",
  pick: "Import files", picking: "Importing files…",
  filter: "Media type", all: "All media", image: "Images", audio: "Audio", video: "Video",
  empty: "No assets match this filter.", loading: "Loading assets…", retry: "Retry",
  loadError: "Assets couldn’t be loaded.", importError: "Some files couldn’t be imported.",
  edit: "Edit notes", notes: "Usage notes", save: "Save notes", cancel: "Cancel",
  noNotes: "No usage notes", notesError: "Notes couldn’t be saved. Try again.",
  pickTitle: "Select media files",
};

export const assetCopy: Record<"en" | "es" | "pt", typeof en> = {
  en,
  es: {
    title: "Recursos", back: "Volver a los perfiles", list: "Recursos del espacio de trabajo",
    pick: "Importar archivos", picking: "Importando archivos…",
    filter: "Tipo de recurso", all: "Todos los recursos", image: "Imágenes", audio: "Audio", video: "Vídeo",
    empty: "No hay recursos que coincidan con este filtro.", loading: "Cargando recursos…", retry: "Reintentar",
    loadError: "No se pudieron cargar los recursos.", importError: "No se pudieron importar algunos archivos.",
    edit: "Editar notas", notes: "Notas de uso", save: "Guardar notas", cancel: "Cancelar",
    noNotes: "Sin notas de uso", notesError: "No se pudieron guardar las notas. Inténtalo de nuevo.",
    pickTitle: "Seleccionar archivos multimedia",
  },
  pt: {
    title: "Recursos", back: "Voltar aos perfis", list: "Recursos do espaço de trabalho",
    pick: "Importar ficheiros", picking: "A importar ficheiros…",
    filter: "Tipo de recurso", all: "Todos os recursos", image: "Imagens", audio: "Áudio", video: "Vídeo",
    empty: "Nenhum recurso corresponde a este filtro.", loading: "A carregar recursos…", retry: "Tentar novamente",
    loadError: "Não foi possível carregar os recursos.", importError: "Não foi possível importar alguns ficheiros.",
    edit: "Editar notas", notes: "Notas de utilização", save: "Guardar notas", cancel: "Cancelar",
    noNotes: "Sem notas de utilização", notesError: "Não foi possível guardar as notas. Tente novamente.",
    pickTitle: "Selecionar ficheiros multimédia",
  },
};
