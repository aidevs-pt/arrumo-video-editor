# Cria e exporta o teu primeiro vídeo

[English](../en/first-video.md) · [Español](../es/first-video.md)

## Configura um espaço de trabalho

Cria um espaço de trabalho e escolhe o agente de edição disponível no teu computador. Se pretendes gerar legendas a partir do áudio, configura um fornecedor de transcrição que devolva marcas temporais. Cada fornecedor precisa das suas próprias credenciais. Cada campo de chave tem **Onde a encontrar**, que abre a página do fornecedor para a criar. No primeiro arranque, o passo do espaço de trabalho também pergunta onde o agente te pode conhecer: adiciona os links do teu canal ou site e, quando o espaço existir, **Contexto** abre com eles e o agente redige o teu contexto e sugere perfis para escolheres.

Cada espaço de trabalho começa com o seu próprio ícone e cor. Para os mudar, ou para mostrar uma imagem como o avatar do teu canal, escolhe **Personalizar espaço de trabalho** no menu do espaço de trabalho.

Em **Definições do espaço de trabalho › Marca**, dá nome a cada cor da marca e usa o visto para escolher se os vídeos a usam; as cores sem visto ficam na paleta, mas não são usadas ao gerar.

Abre **Saídas** e verifica os formatos ativos. Para a entrega ser verificada, cada formato ativo precisa do seu próprio MP4 final e ficheiro SRT.

## Guarda um rascunho e coloca-o na fila

1. Abre **Vídeos** e entra em **Rascunhos**. Adiciona gravações a um rascunho ou escolhe **Começar do zero** e descreve o que queres comunicar com o vídeo.
2. Dá um título ao rascunho e inclui as indicações que influenciam a montagem. Guarda-o para poderes voltar a ele antes de iniciar a produção.
3. Escolhe **Gerar** no rascunho. Nas opções, seleciona os fornecedores e decide se queres usar o **Piloto automático**. O piloto automático avança pelas etapas de revisão. A produção ainda pode ficar à espera de uma renderização ou de ficheiros.
4. Ativa **Gerar automaticamente as legendas em falta** se quiseres que o Arrumo envie o áudio de cada formato renderizado sem SRT para um fornecedor selecionado para a execução. Esta opção está desativada por predefinição.
5. Seleciona **Gerar este vídeo**. Abre **Em produção** para acompanhar a execução.

## Revê e termina a entrega

Numa execução guiada, abre **Rever execução** quando houver uma decisão pendente. Verifica o guião, os recursos ou a pré-visualização disponíveis. Prepara as tuas decisões e envia-as quando terminares. Se precisares de alterações, descreve-as nas notas da etapa antes de enviar a decisão.

Em **Recursos e animações**, o agente também desenha animações para o vídeo, como um cartão de título, oráculos ou um cartão final, e o Arrumo renderiza-as para as reveres. Um vídeo começado do zero não tem gravações, por isso a pré-visualização é um storyboard de cenas animadas feito a partir do guião e das animações aprovadas. As legendas vêm do texto no ecrã. Se houver um fornecedor de texto para voz configurado, como o ElevenLabs, o storyboard é narrado e as legendas seguem a narração. Decide cada animação com **Aprovar**, **Pedir alterações** (com uma nota) ou **Rejeitar**, e marca **Guardar na biblioteca ao aprovar** nas que queres manter. Se aprovares todas e não houver nota geral, o passo é aprovado; caso contrário, volta ao agente uma revisão com cada decisão. Também podes decidir sobre o passo inteiro.

Os formatos que pedem cortes verticais recebem vídeos verticais curtos tirados do vídeo aprovado, cada um com as suas miniaturas, prontos a rever com os títulos e as descrições.

Para veres o que foi pedido ao agente e o que ele mostrou, abre **Registos do agente** junto à atividade da execução. Se um passo falhar, vês aí a saída de erro.

Na etapa de renderização final, usa **Gerar vídeo final** quando estiver disponível. Em **Verificar a entrega**, usa **Gerar legendas** ou **Importar SRT** em cada formato que ainda não tenha legendas. O SRT importado tem de estar em UTF-8 e as marcas temporais devem ficar dentro da duração do vídeo desse formato.

Se a transcrição automática falhar ou for interrompida, podes tentar novamente com **Gerar legendas** ou importar um ficheiro. O Arrumo evita reenviar automaticamente um pedido interrompido porque o fornecedor pode já tê-lo processado. A geração preserva as legendas existentes.

A aplicação de computador verifica a entrega quando todos os formatos ativos têm os ficheiros MP4 e SRT. Se a execução continuar na fila durante a verificação, vê se falta algum ficheiro noutro formato.

## Abre e exporta o resultado

Vai a **Vídeos** → **Concluídos** e abre o vídeo finalizado. Seleciona uma saída para a pré-visualizar ou abre **Texto das legendas** para consultar as suas marcas temporais. Usa **Exportar ZIP** para guardar os ficheiros verificados num novo ZIP. Por baixo do vídeo, **Como foi feito** mostra cada passo com o feedback que deixaste, onde podes comentar para o agente ler quando este vídeo for gerado de novo, além das animações usadas e do registo de atividade. **Definições usadas** permite repetir com as definições capturadas ou atuais e mudar o nome do vídeo.

## Cópia de segurança e restauro

**Definições › Importar e exportar** guarda tudo (definições, espaços de trabalho, perfis, recursos, animações, vídeos terminados e plugins instalados) num `.zip`. As chaves de API não são incluídas: ficam no cofre de credenciais deste computador, por isso volta a adicioná-las num computador novo.

Para restaurar, escolhe uma cópia no mesmo sítio. O Arrumo verifica primeiro cada ficheiro com as somas da cópia, depois mostra o que contém (espaços de trabalho, plugins, tamanho) e pergunta antes de substituir alguma coisa; **Exportar os meus antes** guarda os teus dados atuais antes de decidires. Reinicia para a restaurar e guarda os dados substituídos numa pasta chamada `workspaces.sqlite.before-import-…` junto aos teus dados, caso precises deles.

## Dados e privacidade

O Arrumo conta que funcionalidades são usadas e reporta erros, para saber o que melhorar. Ambas as opções estão ativas por omissão e podem ser desligadas em **Definições › Geral › Dados e privacidade**. Estão ligadas a um identificador aleatório desta instalação, nunca a ti, e nunca incluem os teus vídeos, ficheiros, textos ou chaves. As compilações que fazes tu não enviam nada.

## Créditos dos fornecedores

Definições › Serviços e chaves mostra o que resta a cada chave quando o plugin consegue saber. Com 20% restantes aparece um aviso amarelo no topo, e com 10% um vermelho; se o fechar, só volta no nível seguinte. A página do rascunho repete o aviso acima de **Gerar este vídeo**, porque um fornecedor esgotado para o vídeo nesse passo.

## Plugins novos

Quando chegam plugins, de um ZIP, do registo ou de uma atualização do Arrumo, o Arrumo apresenta-os numa visita curta, uma página por plugin. Podes saltá-la quando quiseres; **Definições › Plugins › Ver a visita aos plugins** volta a mostrar a apresentação de cada um.
