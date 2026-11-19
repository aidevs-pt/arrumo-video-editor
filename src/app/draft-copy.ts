const en = {
  title: "Drafts", back: "Back to profiles", newDraft: "New video draft", empty: "No video drafts yet.",
  name: "Video title", profile: "Profile", noProfile: "No profile", intent: "Intent and guidance",
  sources: "Source clips", order: "Playback order", import: "Import clips", up: "Move up", down: "Move down",
  save: "Save draft", saving: "Saving…", loading: "Loading drafts…", retry: "Retry",
  loadError: "Drafts couldn’t be loaded.", saveError: "The draft couldn’t be saved.",
  importError: "Some clips couldn’t be imported.", intentHint: "Add a clip or describe the video you want to make.",
};
export const draftCopy: Record<"en" | "es" | "pt", typeof en> = {
  en,
  es: {
    title: "Borradores", back: "Volver a los perfiles", newDraft: "Nuevo borrador", empty: "Aún no hay borradores.",
    name: "Título del vídeo", profile: "Perfil", noProfile: "Sin perfil", intent: "Intención e indicaciones",
    sources: "Clips de origen", order: "Orden de reproducción", import: "Importar clips", up: "Subir", down: "Bajar",
    save: "Guardar borrador", saving: "Guardando…", loading: "Cargando borradores…", retry: "Reintentar",
    loadError: "No se pudieron cargar los borradores.", saveError: "No se pudo guardar el borrador.",
    importError: "No se pudieron importar algunos clips.", intentHint: "Añade un clip o describe el vídeo que quieres crear.",
  },
  pt: {
    title: "Rascunhos", back: "Voltar aos perfis", newDraft: "Novo rascunho", empty: "Ainda não há rascunhos.",
    name: "Título do vídeo", profile: "Perfil", noProfile: "Sem perfil", intent: "Intenção e orientações",
    sources: "Clipes de origem", order: "Ordem de reprodução", import: "Importar clipes", up: "Mover para cima", down: "Mover para baixo",
    save: "Guardar rascunho", saving: "A guardar…", loading: "A carregar rascunhos…", retry: "Tentar novamente",
    loadError: "Não foi possível carregar os rascunhos.", saveError: "Não foi possível guardar o rascunho.",
    importError: "Não foi possível importar alguns clipes.", intentHint: "Adicione um clipe ou descreva o vídeo que pretende criar.",
  },
};
