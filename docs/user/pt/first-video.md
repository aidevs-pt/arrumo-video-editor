# Cria e exporta o teu primeiro vídeo

[English](../en/first-video.md) · [Español](../es/first-video.md)

## Configura um espaço de trabalho

Cria um espaço de trabalho e escolhe o agente de edição disponível no teu computador. Se pretendes gerar legendas a partir do áudio, configura um fornecedor de transcrição que devolva marcas temporais. Cada fornecedor precisa das suas próprias credenciais.

Abre **Saídas** e verifica os formatos ativos. Para a entrega ser verificada, cada formato ativo precisa do seu próprio MP4 final e ficheiro SRT.

## Guarda um rascunho e coloca-o na fila

1. Abre **Vídeos** e entra em **Rascunhos**. Adiciona gravações a um rascunho ou escolhe **Começar do zero** e descreve o que queres comunicar com o vídeo.
2. Dá um título ao rascunho e inclui as indicações que influenciam a montagem. Guarda-o para poderes voltar a ele antes de iniciar a produção.
3. Escolhe **Gerar** no rascunho. Nas opções, seleciona os fornecedores e decide se queres usar o **Piloto automático**. O piloto automático avança pelas etapas de revisão. A produção ainda pode ficar à espera de uma renderização ou de ficheiros.
4. Ativa **Gerar automaticamente as legendas em falta** se quiseres que o Arrumo envie o áudio de cada formato renderizado sem SRT para um fornecedor selecionado para a execução. Esta opção está desativada por predefinição.
5. Seleciona **Gerar este vídeo**. Abre **Em produção** para acompanhar a execução.

## Revê e termina a entrega

Numa execução guiada, abre **Rever execução** quando houver uma decisão pendente. Verifica o guião, os recursos ou a pré-visualização disponíveis. Prepara as tuas decisões e envia-as quando terminares. Se precisares de alterações, descreve-as nas notas da etapa antes de enviar a decisão.

Em **Recursos e animações**, o agente também desenha animações para o vídeo, como um cartão de título, oráculos ou um cartão final, e o Arrumo renderiza-as para as reveres. Um vídeo começado do zero não tem gravações, por isso a pré-visualização é um storyboard de cenas animadas feito a partir do guião e das animações aprovadas. As legendas vêm do texto no ecrã. Se houver um fornecedor de texto para voz configurado, como o ElevenLabs, o storyboard é narrado e as legendas seguem a narração.

Para veres o que foi pedido ao agente e o que ele mostrou, abre **Registos do agente** junto à atividade da execução. Se um passo falhar, vês aí a saída de erro.

Na etapa de renderização final, usa **Gerar vídeo final** quando estiver disponível. Em **Verificar a entrega**, usa **Gerar legendas** ou **Importar SRT** em cada formato que ainda não tenha legendas. O SRT importado tem de estar em UTF-8 e as marcas temporais devem ficar dentro da duração do vídeo desse formato.

Se a transcrição automática falhar ou for interrompida, podes tentar novamente com **Gerar legendas** ou importar um ficheiro. O Arrumo evita reenviar automaticamente um pedido interrompido porque o fornecedor pode já tê-lo processado. A geração preserva as legendas existentes.

A aplicação de computador verifica a entrega quando todos os formatos ativos têm os ficheiros MP4 e SRT. Se a execução continuar na fila durante a verificação, vê se falta algum ficheiro noutro formato.

## Abre e exporta o resultado

Vai a **Vídeos** → **Concluídos** e abre o vídeo finalizado. Seleciona uma saída para a pré-visualizar ou abre **Texto das legendas** para consultar as suas marcas temporais. Usa **Exportar ZIP** para guardar os ficheiros verificados num novo ZIP.
