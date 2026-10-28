# QueryMass

Aplicativo desktop para executar SQL em várias conexões MySQL e conferir resultados por máquina.

## Requisitos e instalação

Use Rust estável com Cargo para compilar. No Windows, use a toolchain MSVC com as ferramentas de compilação do Visual Studio. No Linux, é necessária uma sessão gráfica com suporte a OpenGL.

```sh
cargo build --release
```

O executável fica em `target/release/querymass` no Linux e `target/release/querymass.exe` no Windows. Os workflows de CI compilam e disponibilizam artefatos para os dois sistemas.

## Uso

1. Abra **Conexões** e cadastre uma máquina ou importe uma planilha. O teste de conexão não executa o SQL do editor.
2. Marque máquinas ou pastas no painel esquerdo. Use a busca para filtrar nome, IP, banco ou pasta. Conexões novas entram desmarcadas.
3. Escreva SQL e clique em **Executar** ou pressione `Ctrl+Enter`. Se houver uma seleção de texto, somente esse trecho é executado.
4. Escolha uma máquina em **Resultados** para conferir suas tabelas. **Mensagens** mostra progresso e erros. Copie células ou linhas pelo botão direito e exporte a tabela para CSV.

O modo **Somente leitura** usa instruções preparadas e uma transação `READ ONLY` no servidor. Consultas incompatíveis são bloqueadas. Para alterar dados, desative esse modo e revise o SQL e os alvos na confirmação. A confirmação usa uma cópia dos alvos e do SQL.

Cada servidor executa de forma independente. Não há transação distribuída nem repetição automática. Cancelamento e prazo máximo solicitam `KILL QUERY` em uma conexão separada. Uma escrita interrompida ou desconectada pode ter sido aplicada: o status **Resultado desconhecido** exige conferir o banco antes de repetir.

## Conexões e pastas

O gerenciador reúne cadastro, importação, credenciais compartilhadas e pastas. Uma conexão usa credenciais próprias ou compartilhadas, explicitamente. Uma senha própria vazia permanece vazia.

Máquinas sem pasta ficam na raiz. Crie pastas vazias no gerenciador e arraste máquinas para organizá-las. O cadastro também permite escolher a pasta pelo teclado. Excluir uma pasta move suas máquinas para a raiz; excluir uma conexão remove apenas o cadastro local.

TLS é configurado por conexão. Ao habilitá-lo, a cadeia e o nome do servidor são validados. É possível escolher uma CA em PEM. Conexões sem TLS são identificadas na configuração.

## Importação

Arquivos aceitos: CSV, XLSX e XLSM. Excel usa a primeira aba; CSV reconhece vírgula, ponto e vírgula, tabulação e `|`. A prévia mostra conexões válidas, repetidas, linhas sem IP e erros antes de aplicar.

| Coluna | Conteúdo |
| --- | --- |
| `IP` | Endereço IP ou DNS obrigatório; também aceita `HOST`, `HOSTNAME`, `ADDRESS`, `ENDPOINT` e `TARGET` |
| `HOSTNAME` | Nome exibido na lista |
| `LOCATION` | Pasta; também aceita `PASTA`, `LOCAL` e `GRUPO` |
| `PORT` | Porta; padrão `3306` |
| `DATABASE` | Banco padrão; pode ficar vazio para SQL com nomes qualificados |
| `USER` | Usuário próprio; se ausente, usa credenciais compartilhadas |
| `PASSWORD` | Senha própria; com usuário próprio, vazio significa senha vazia |

Duplicatas consideram endereço, porta, banco e usuário. Linhas inválidas não descartam as válidas. O botão **Baixar modelo CSV** fornece um arquivo de exemplo sem credenciais.

## Persistência e credenciais

Conexões, pastas, editor e histórico são salvos automaticamente em:

- Linux: `$XDG_CONFIG_HOME/querymass/workspace.json` ou `~/.config/querymass/workspace.json`.
- Windows: `%APPDATA%\querymass\workspace.json`.

O arquivo não contém os campos de senha. **Guardar senha no cofre do sistema** usa Secret Service no Linux e o cofre nativo no Windows. Sem essa opção, a senha permanece na sessão. Se o cofre estiver indisponível, o aplicativo informa a falha e não usa uma senha vazia como substituta de uma senha que não conseguiu recuperar.

O histórico contém SQL e resumos de até 30 execuções, sem dados retornados. Consultas que contêm termos de autenticação como `PASSWORD`, `IDENTIFIED`, `TOKEN`, `SECRET` ou `SENHA` não são persistidas automaticamente. Arquivos SQL e CSV exportados são gravados por solicitação do usuário e podem conter dados sensíveis.

A gravação usa arquivo temporário e substituição do arquivo anterior. No Linux, o arquivo é criado com permissão `0600`. Uma trava impede duas instâncias de sobrescrever o mesmo espaço de trabalho. Configurações inválidas são preservadas e oferecem recuperação com cópia do original.

`QUERYMASS_CONFIG` permite escolher outro arquivo de configuração, com credenciais isoladas por espaço de trabalho.

## Limites

- Até 16 máquinas em paralelo; padrão 4.
- Até 100 instruções por lote.
- Padrão de 1.000 linhas por resultado, ajustável até 10.000.
- Limite de memória dos dados: 4 MiB por máquina e 64 MiB por execução, considerando dados e estruturas principais.
- `SQL_SELECT_LIMIT` reduz consultas sem limite explícito; cláusulas `LIMIT` explícitas e algumas instruções do servidor podem exigir mais leitura de rede. Restringir o SQL continua sendo necessário para consultas grandes.
- Resultados permanecem na sessão até outra execução. Use CSV para guardar os dados.
- O modo de leitura exige suporte do servidor a transações `READ ONLY`. Comandos de cliente como `DELIMITER` não são aceitos.
- O prazo da consulta inicia após a conexão e configuração da sessão. Há também timeout de conexão de 5 segundos e timeout de rede.

## Desenvolvimento e verificações

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Os testes de integração MySQL são ignorados por padrão. Configure `QM_TEST_HOST`, `QM_TEST_USER`, `QM_TEST_PASSWORD` e `QM_TEST_DATABASE` por um mecanismo privado antes de executar:

```sh
cargo test --test mysql_integration -- --ignored --test-threads=1
```

Esses testes criam apenas tabelas temporárias, verificam alterações, múltiplos resultados, falhas parciais, cancelamento, prazo e recusa de TLS não confiável. O teste opcional `system_keyring_roundtrip` usa e remove uma credencial exclusiva de teste.

A feature `ui-qa`, desabilitada nas compilações comuns, permite capturas do framebuffer e sequências de entrada por arquivos em `QUERYMASS_QA_CAPTURE`. Ela existe para verificar telas e interações sem depender de captura do desktop.

## Dependências

`eframe` e `egui_extras` fornecem a interface e a tabela virtualizada; `mysql` fornece o protocolo; `calamine` e `csv` leem planilhas; `rfd` abre diálogos nativos; `serde` e `serde_json` persistem configurações; `keyring` integra o cofre do sistema. A separação entre motor, persistência, importação e interface permite verificar as regras sem abrir a janela.
