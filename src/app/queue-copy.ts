const en = {
  title: "Generation queue", back: "Back to drafts", submit: "Queue generation",
  runs: "Runs", empty: "No generation runs yet.", loading: "Loading generation runs…",
  loadError: "Generation runs couldn’t be loaded.", retry: "Retry",
  options: "Run options", platform: "Publishing platform (optional)", autopilot: "Autopilot",
  submitting: "Submitting…", submitError: "The draft couldn’t be queued.",
  queued: "Queued", running: "Running", paused: "Paused", failed: "Failed",
  completed: "Completed", cancelled: "Cancelled",
};
export const queueCopy: Record<"en" | "es" | "pt", typeof en> = {
  en,
  es: {
    title: "Cola de generación", back: "Volver a los borradores", submit: "Añadir a la cola",
    runs: "Ejecuciones", empty: "Aún no hay ejecuciones.", loading: "Cargando ejecuciones…",
    loadError: "No se pudieron cargar las ejecuciones.", retry: "Reintentar",
    options: "Opciones de generación", platform: "Plataforma de publicación (opcional)", autopilot: "Piloto automático",
    submitting: "Enviando…", submitError: "No se pudo añadir el borrador a la cola.",
    queued: "En cola", running: "En ejecución", paused: "En pausa", failed: "Error",
    completed: "Completada", cancelled: "Cancelada",
  },
  pt: {
    title: "Fila de geração", back: "Voltar aos rascunhos", submit: "Adicionar à fila",
    runs: "Execuções", empty: "Ainda não há execuções.", loading: "A carregar as execuções…",
    loadError: "Não foi possível carregar as execuções.", retry: "Tentar novamente",
    options: "Opções de geração", platform: "Plataforma de publicação (opcional)", autopilot: "Piloto automático",
    submitting: "A enviar…", submitError: "Não foi possível adicionar o rascunho à fila.",
    queued: "Em fila", running: "Em execução", paused: "Em pausa", failed: "Falhou",
    completed: "Concluída", cancelled: "Cancelada",
  },
};
