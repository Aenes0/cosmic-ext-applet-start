# Iniciar — menu Iniciar do Windows 11 para o COSMIC

Um applet para o painel do COSMIC: um menu Iniciar como o do Windows 11, que toma o
lugar do lançador da tecla Super e da biblioteca de apps, os dois num menu só.

No tema escuro ou claro do sistema, numa página só, de cima para baixo:

- a **pesquisa**, já selecionada (é só digitar): a melhor correspondência em cima, os
  apps, as janelas e os arquivos embaixo, e à direita o escolhido, grande, com o que dá
  para fazer com ele (Abrir, Nova janela, Fixar…); as setas e o Enter escolhem;
- **Fixados**, em duas linhas de oito, com **Mostrar tudo** para o resto;
- **Abertos**: as janelas abertas; clicar vai para a janela;
- **Recomendados**: os apps instalados há pouco ("Adicionado recentemente"), os abertos
  pelo menu e os arquivos mexidos há pouco ("Há 3 h"), com **Mostrar tudo**. Os arquivos
  vêm do `recently-used.xbel` (também o dos Flatpaks) e dos mais novos em Área de
  trabalho, Downloads, Documentos, Imagens, Vídeos e Música, porque os apps do COSMIC
  não registram o que abrem;
- **Todos**, em **Exibir: Categoria** (pastas com quatro ícones, que abrem por cima,
  como no Windows), **Grade** ou **Lista**;
- no rodapé, o usuário (Alterar configurações da conta, Bloquear, Sair), Arquivos,
  Configurações e o botão de energia (Bloquear, Suspender, Desligar, Reiniciar).

O clique direito num app abre o menu dele: Abrir, as ações do `.desktop` e
Fixar/Desafixar do Iniciar; numa janela, Ir para, Nova janela e Fechar; num arquivo, Abrir
e Abrir local do arquivo. Esc fecha um menu, depois limpa a pesquisa, depois fecha tudo;
clicar fora também fecha. Na primeira vez, os fixados são os do dock, mais Arquivos,
Terminal, Configurações, Loja e Editor de Texto.

## Clique direito no botão (Win+X)

O clique direito no botão Iniciar, ou Super+X, abre o menu de ferramentas do Windows, na
mesma ordem, cada uma com o que faz isso aqui:

| No Windows | Aqui |
| --- | --- |
| Aplicativos instalados | Configurações › Aplicativos |
| Opções de energia | Configurações › Energia |
| Visualizador de eventos | `journalctl -b` num terminal |
| Sistema | Configurações › Sobre |
| Conexões de rede | Configurações › Rede |
| Gerenciamento de disco | GNOME Discos, quando instalado |
| Terminal | COSMIC Terminal |
| Terminal (Admin) | `sudo -i` num terminal |
| Gerenciador de Tarefas | COSMIC Monitor (ou `btop` num terminal) |
| Configurações | Configurações |
| Explorador de Arquivos | Arquivos do COSMIC |
| Pesquisar | o menu Iniciar |
| Executar | o lançador do COSMIC |
| Desligar ou sair › | Sair, Suspender, Desligar, Reiniciar |
| Área de trabalho | minimiza todas as janelas |

O que não tem equivalente (Gerenciador de dispositivos, Central de mobilidade,
Gerenciamento do computador) fica de fora. O COSMIC Terminal não roda um comando que se
passa a ele, então o Visualizador de eventos e o Terminal (Admin) abrem no primeiro destes
que houver: Alacritty, kitty, foot, WezTerm, Konsole, GNOME Terminal, xterm. O Super+X,
que no COSMIC troca janelas de lugar, passa a abrir este menu (`./setup.sh --undo` devolve).

## Como funciona

São dois papéis do mesmo programa:

- `cosmic-ext-applet-start`, como o painel roda os applets, é o **botão** de quatro
  quadrados;
- `cosmic-ext-applet-start --tools` abre as ferramentas (Super+X), e `--show-desktop`
  minimiza todas as janelas;
- `cosmic-ext-applet-start --menu` é o **menu**, uma camada sobre a tela (não um popup de
  applet), para a tecla Super poder abri-lo e ele receber o teclado. O primeiro fica
  aberto, escondido entre um uso e outro; os seguintes, e o botão, só pedem a ele, pelo
  D-Bus (`io.github.Aenes0.CosmicStart`), para abrir ou fechar.

As janelas abertas vêm do protocolo de toplevels do COSMIC; os apps, dos arquivos
`.desktop`; os fixados e os apps usados ficam na configuração do COSMIC
(`io.github.Aenes0.CosmicStart.Menu`). Quem usava o menu dentro do
[Central](https://github.com/Aenes0/cosmic-ext-applet-central) leva os fixados junto.

## Instalar

```sh
./install.sh            # compila e instala em ~/.local
./setup.sh              # tecla Super abre o menu; o botão Iniciar vai para o painel
```

`./setup.sh` faz a tecla Super (e Super+A) abrir o menu e põe o botão **Iniciar** no
lugar dos botões do lançador e da biblioteca no painel; `./setup.sh --undo` volta os do
COSMIC. Dá também para adicionar só o botão em Configurações › Área de trabalho › Painel ›
Applets.

Precisa de Rust 1.93 ou mais novo (o libcosmic pede) e das bibliotecas de
desenvolvimento do Wayland e do xkbcommon.

`COSMIC_START_PREVIEW` (`recommended`, `power`, `user`, `tools-power`, `grid`, `list`, `end`, o número
de uma categoria ou um texto para pesquisar) abre o menu já ali, para tirar print.

O menu fala português e inglês, conforme o idioma do sistema. Licença GPL-3.0.
