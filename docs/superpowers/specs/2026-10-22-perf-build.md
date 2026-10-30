# Performance e economia do build e do binário — medições e proposta

Data: 2026-10-22 · Escopo: só medição (nada no repo foi alterado; experimentos em `target/exp-*`).
Máquina: 32 núcleos, rustc/cargo 1.99.1 (Arch), linker LLD 23 (padrão do rustc).

## 1. Baseline (perfil atual: `opt-level=3`, `lto="thin"`, `panic="abort"`)

| Medida | Valor |
|---|---|
| Binário release (= instalado em `~/.local/bin`, `cmp` idêntico) | **35 750 128 B (35,75 MB)** |
| Desse total, só símbolos (`.symtab` 1,2 MB + `.strtab` 7,6 MB) | ~8,8 MB — não há seções `.debug_*` (`debug=false` já é o padrão) |
| `.text` / `.rodata` / `.eh_frame` | 18,47 MB / 4,00 MB / 1,65 MB |
| Build limpo (deps já baixadas, `--offline`) | **76 s** |
| Incremental (toca `src/main.rs`) | **27 s** (outra amostra: 47 s) |
| Crates no grafo (normal, únicos) / com build+dev | 452 / 461 |
| Duplicados (`cargo tree -d`) | 76 entradas — todas vindas do grafo do libcosmic (3× xkbcommon, 3× hashbrown, 3× linux-raw-sys, 2× png, 2× zune-jpeg, 2× rustix, 2× thiserror…) |
| Bibliotecas dinâmicas (`ldd`) | 5: libxkbcommon, libgcc_s, libm, libc, ld-linux (GPU/Vulkan são carregadas por `dlopen` só no menu) |

Maiores contribuintes de `.text` (agregação por crate via `nm -S`, aproximada — genéricos ficam em `core`/`alloc`):
core 1,92 MB · **naga 1,45** · alloc 1,01 · **wgpu_core 0,83** · std 0,60 · hashbrown 0,55 · ron 0,53 · zbus 0,53 ·
wayland_client 0,52 · app 0,44 · iced_winit 0,44 · cosmic 0,43 · **wgpu_hal 0,37** · read_fonts/skrifa 0,75 ·
zune_jpeg 0,29 · **winit_x11 0,26** · tokio 0,26 · image_webp 0,19 · tiny_skia 0,19.

## 2. Perfis (mesma árvore de código; `--config profile.release.*`)

| Variante | Binário | Após `strip` | `.text` | Build limpo | Incremental |
|---|---:|---:|---:|---:|---:|
| **base** (o3 + thin) | 35,75 MB | 26,94 MB | 18,47 MB | 76 s | 27 s |
| `lto=false` (thin-local) | 35,33 | 25,95 | 17,12 | 98 s | — |
| `lto="fat"` | 27,96 | 23,92 | 16,99 | 263 s | — |
| `lto="fat"` + `codegen-units=1` | 26,30 | **22,58** | 15,90 | 425 s | **202 s** |
| `opt-level="s"` (thin) | 33,32 | 21,63 | 12,45 | 72 s | — |
| `opt-level="s"` + fat + cgu1 | 21,71 | 17,62 | 11,00 | 194 s | — |
| `opt-level="z"` + fat + cgu1 | 21,40 | **16,30** | 9,09 | 172 s | — |

- `strip="symbols"` sozinho: **−8,8 MB (−25 %)** sem custo de build nem de execução (só some o nome das funções em backtrace de panic).
- Linker: já é LLD; forçar `-fuse-ld=lld` deu binário de md5 idêntico (0 ganho). `mold` não está instalado.
- fat+cgu1 multiplica o incremental por ~7,5× (27 s → 202 s): inviável no ciclo de desenvolvimento, aceitável só para instalar.

## 3. Feature `wgpu`

Cópia sem `"wgpu"` nas features do libcosmic (o crate `iced_wgpu` continua no grafo porque o libcosmic o ativa por `wayland`; só o backend sai):

| | Binário | Após strip | `.text` | Crates |
|---|---:|---:|---:|---:|
| com wgpu (base) | 35,75 MB | 26,94 MB | 18,47 MB | 452 |
| sem wgpu | 28,12 MB | 21,26 MB | 14,15 MB | 448 |

O app compila e roda só com tiny-skia (o botão já usa: `main.rs` força `ICED_BACKEND=tiny-skia`). Mas o commit
`e3ba73a` mediu: **menu no tiny-skia rolava a 60 % de um núcleo e engasgava; no wgpu fica leve** (RSS 11 MB → 38 MB
"só dele"). Transparência, sombras (`iced::Shadow`) e miniaturas funcionam nos dois; blur já está desligado
(`set_auto_blur(Default::default())`).

Medido agora (processos em execução, só leitura de `/proc`):

| Processo | RSS | Anônima | Arquivo | PSS | Páginas do executável | Mapas de GPU |
|---|---:|---:|---:|---:|---:|---:|
| botão (×3, um por monitor) | 8,1–9,6 MB | 20–1224 KB | ~8,1 MB | 1,5–2,9 MB | ~4,3 MB (compartilhadas) | **0** |
| menu (`--menu`) | 110 MB | 22 MB | 78 MB | 75 MB | 22 MB | 90 |

Conclusão: o botão **não paga** o wgpu em RAM (nenhuma biblioteca de GPU mapeada); tirar a feature só economiza
disco (−5,7 MB) e piora a rolagem do menu. **Não recomendado.** Os 78 MB do menu são majoritariamente bibliotecas
do driver (arquivo, recuperáveis), não memória anônima.

## 4. Dependências ociosas (Grep + `cargo tree -e features`)

| Dep | Uso real no código | Teste | Resultado |
|---|---|---|---|
| tokio `full` | `runtime::Builder::new_current_thread`, `task::spawn_blocking`, `time::sleep` | `features=["rt","time"]` compila | −1 crate (tokio-macros); tamanho +3 KB (ruído); build igual. libcosmic/zbus/iced já ligam `rt-multi-thread`, `fs`, `net`, `process`, `io-util`; só `signal`, `parking_lot`, `io-std`, `macros` vinham de nós |
| tracing-subscriber | `fmt` + `EnvFilter` | sem `ansi` (default-features=false + fmt,std,env-filter,tracing-log,smallvec) | −1 crate (nu-ansi-term); +60 KB (ruído) |
| tokio min + ts min juntos | — | compila | −2 crates; tamanho 35,79 MB (+41 KB, ruído de LTO); build igual |
| chrono | `DateTime::parse_from_rfc3339` | já `default-features=false, ["clock"]` | nada a fazer |
| image | `RgbaImage`, `imageops::thumbnail` | já `default-features=false` | jpeg/png/bmp/ico são ligados por libcosmic/iced, fora do nosso controle |
| futures | `executor::block_on`, `future::pending`, `channel::mpsc`, `SinkExt` | default necessário (executor) | nada a fazer |
| zbus, memmap2, rustix, shlex, libc | usados | — | nada a fazer |
| winit X11 (0,26 MB de `.text`) | app é só Wayland | ligado pela feature `winit` do libcosmic | fora do nosso controle (hipótese: só via fork/patch do libcosmic) |

`cargo machete`/`udeps`/`bloat` não estão instalados; a análise foi manual. Não há dependência direta sem uso.

## 5. Partida do processo (proxy isolado)

Sem compositor nem D-Bus (`env -i`, `HOME` e `XDG_RUNTIME_DIR` vazios, `WAYLAND_DISPLAY` inexistente), modo botão:
o processo carrega, lê tema/fontes e aborta em "Create event loop: NoCompositor". 20 execuções por variante:

| Variante | Média até abortar | Loader dinâmico (`LD_DEBUG=statistics`) | Relocações |
|---|---:|---:|---:|
| base | 158 ms (2 rodadas) | 0,5–2,7 M ciclos (< 1 ms) | 504 |
| fat+cgu1 | 144 ms | 1,3 M ciclos | 503 |
| s+fat+cgu1 | 147 ms | 1,2 M | 503 |
| z+fat+cgu1 | 153 ms | 1,7 M | 503 |
| sem wgpu | 153 ms | 1,1 M | 481 |
| fat+cgu1+strip+deps min | 139 ms | 0,3 M | 503 |

O loader é desprezível (< 1 ms, só 5 libs). Os ~140–160 ms são de inicialização (tema, sistema de fontes —
hipótese, não perfilado) e a diferença entre variantes (~10 %) está perto do ruído com n=20.

## 6. Ranking de ajustes

| # | Ajuste | Ganho medido | Custo de build | Risco | Esforço |
|---|---|---|---|---|---|
| 1 | `strip = "symbols"` no release | −8,8 MB (35,75 → 26,94) | 0 | Muito baixo: backtrace de panic sem nomes (o app usa `panic=abort`) | 1 linha |
| 2 | Perfil `dist` (fat + cgu1 + strip) só para instalar | −13,2 MB (35,75 → 22,58, −37 %); `.text` −14 %; partida −5..−12 % (ruído) | limpo 425 s, incremental 202 s — **só no install** | Baixo: mesmo código, mesmo opt-level | perfil + 1 linha no `install.sh` |
| 3 | tokio `["rt","time"]`, tracing-subscriber sem `ansi` | −2 crates, 0 bytes, 0 s | 0 | Baixo; sem `ansi` o log fica sem cor (no journal isso é até melhor) | 2 linhas |
| 4 | `opt-level="s"`/`"z"` | −18,3 / −19,6 MB (vs base) | ~ igual ao fat | **Médio/alto, não medido**: tiny-skia (botão), layout e rolagem podem ficar mais lentos — o gargalo já medido é layout | Só com medição de quadro antes |
| 5 | Tirar feature `wgpu` | −5,7 MB após strip | −poucos s | **Alto**: volta a rolagem a 60 % de um núcleo (commit e3ba73a) | 1 linha — **não fazer** |
| — | `-fuse-ld=lld` / `debug=false` | 0 (já é o padrão) | — | — | — |

## 7. Diff proposto do `Cargo.toml`

```diff
 [dependencies]
-tokio = { version = "1.49.0", features = ["full"] }
-tracing-subscriber = { version = "0.3.22", features = ["env-filter"] }
+tokio = { version = "1.49.0", features = ["rt", "time"] }
+tracing-subscriber = { version = "0.3.22", default-features = false, features = ["fmt", "std", "env-filter", "tracing-log", "smallvec"] }

 [profile.release]
 opt-level = 3
 panic = "abort"
 lto = "thin"
+strip = "symbols"
+
+# O que se instala: menor e um pouco mais enxuto, sem pesar no ciclo de desenvolvimento.
+# cargo build --profile dist  ->  target/dist/cosmic-ext-applet-start
+[profile.dist]
+inherits = "release"
+lto = "fat"
+codegen-units = 1
```

E no `install.sh`: `cargo build --profile dist` e `install -Dm0755 "target/dist/$bin" ...`
(o `Cargo.lock` muda ao aplicar o item 3: saem `tokio-macros` e `nu-ansi-term`).

**O que muda para o usuário:** binário instalado de 35,75 MB → ~22,6 MB; ~8,6 MB a menos de código mapeável
no menu (hipótese: alguns MB a menos de páginas de arquivo residentes, compartilhadas e recuperáveis); instalação
leva ~7 min em vez de ~1 min do zero (~3,5 min incremental). O `cargo build --release` do dia a dia continua 76 s/27 s.
**O que NÃO muda:** visual (mesmo renderizador: botão tiny-skia, menu wgpu), comportamento, RSS anônimo
(memória de verdade), threads, D-Bus, nº de bibliotecas carregadas.

## 8. Não medido / limites

- RSS e tempo de quadro do **menu** por variante: rodar `--menu` de teste abriria o menu na tela e roda
  `menu::migrate()` (grava configuração); não foi feito. O menu atual não estava rodando no início da medição.
- Partida real até o "primeiro estado pronto": sem `strace`/`ltrace`; o journal não tem marca de início. Só o proxy da §5.
- Impacto de `opt-level="s"/"z"` em rolagem/layout: exige a sonda de quadro (ver memória `painel-recarga`).
- `cargo bloat`, `cargo machete`, `cargo udeps`, `bloaty`, nightly: não instalados.

## 9. Efeitos colaterais da medição

- **~154 core dumps** (≈1 MB cada) em `/var/lib/systemd/coredump`, das 20×7 execuções do proxy (o app aborta por
  `panic="abort"`). Expiram pelo `systemd-tmpfiles` (padrão 3 dias) ou `coredumpctl`; não foram apagados.
- **~20 GB** em `target/exp-*` (diretórios de build dos experimentos). Podem ser removidos à mão.
- Observação lateral: o journal do menu recebe rajadas de `WARN usvg::parser::svgtree: Failed to parse marker-*`
  (dezenas de linhas por segundo ao abrir); um filtro `usvg=error` no `EnvFilter` padrão cortaria isso (não medido).
