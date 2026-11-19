export const profileCopy = {
  en: {
    back: "Back to workspaces",
    title: "Profiles",
    empty: "No profiles yet",
    emptyHint: "Create a profile for a recurring type of video, such as reviews.",
    loading: "Loading profiles…",
    loadError: "Profiles couldn’t be loaded.",
    createError: "The profile couldn’t be saved. Try again.",
    retry: "Retry",
    form: "New profile",
    name: "Profile name",
    guidance: "Editing guidance (optional)",
    color: "Color (optional)",
    colorHint: "Use a hex color such as #532A9D.",
    submit: "Create profile",
    saving: "Creating…",
  },
  es: {
    back: "Volver a los espacios de trabajo",
    title: "Perfiles",
    empty: "Aún no hay perfiles",
    emptyHint: "Crea un perfil para un tipo de vídeo recurrente, como las reseñas.",
    loading: "Cargando perfiles…",
    loadError: "No se pudieron cargar los perfiles.",
    createError: "No se pudo guardar el perfil. Inténtalo de nuevo.",
    retry: "Reintentar",
    form: "Nuevo perfil",
    name: "Nombre del perfil",
    guidance: "Indicaciones de edición (opcional)",
    color: "Color (opcional)",
    colorHint: "Usa un color hexadecimal, como #532A9D.",
    submit: "Crear perfil",
    saving: "Creando…",
  },
  pt: {
    back: "Voltar aos espaços de trabalho",
    title: "Perfis",
    empty: "Ainda não há perfis",
    emptyHint: "Crie um perfil para um tipo de vídeo recorrente, como análises.",
    loading: "A carregar perfis…",
    loadError: "Não foi possível carregar os perfis.",
    createError: "Não foi possível guardar o perfil. Tente novamente.",
    retry: "Tentar novamente",
    form: "Novo perfil",
    name: "Nome do perfil",
    guidance: "Orientações de edição (opcional)",
    color: "Cor (opcional)",
    colorHint: "Use uma cor hexadecimal, como #532A9D.",
    submit: "Criar perfil",
    saving: "A criar…",
  },
} satisfies Record<"en" | "es" | "pt", Record<keyof {
  back: string;
  title: string;
  empty: string;
  emptyHint: string;
  loading: string;
  loadError: string;
  createError: string;
  retry: string;
  form: string;
  name: string;
  guidance: string;
  color: string;
  colorHint: string;
  submit: string;
  saving: string;
}, string>>;
