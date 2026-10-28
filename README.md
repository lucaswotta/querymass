# QueryMass

Aplicativo desktop para escrever consultas SQL e executá-las em várias máquinas com MySQL.

## Requisitos

- Rust e Cargo
- Linux ou Windows

## Executar

```sh
cargo run
```

## Estado

A interface divide o aplicativo entre a lista de máquinas, o editor SQL e a área de saída. Máquinas podem ser cadastradas manualmente ou importadas, agrupadas por pasta e selecionadas individualmente ou em grupo. As credenciais e a lista permanecem apenas na memória durante a sessão.

## Importar máquinas

O CSV, XLSX ou XLSM deve ter uma máquina por linha. O cabeçalho precisa incluir `IP`, `HOST`, `HOSTNAME`, `ADDRESS`, `ENDPOINT` ou `TARGET`. `HOSTNAME` também pode definir o nome mostrado; `LOCATION` agrupa máquinas em pastas.

| Coluna | Uso |
| --- | --- |
| `IP` | Obrigatória. IP ou nome DNS do servidor MySQL. As colunas `HOST`, `HOSTNAME`, `ADDRESS`, `ENDPOINT` e `TARGET` também são aceitas. |
| `HOSTNAME` | Nome exibido na lista; se ausente, usa o IP. |
| `LOCATION` | Pasta para agrupar e selecionar máquinas em conjunto. |
| `PORT` | Porta MySQL; padrão `3306`. |
| `DATABASE` | Banco padrão da conexão; pode ficar vazio. |
| `USER` | Usuário específico da máquina; substitui o padrão. |
| `PASSWORD` | Senha específica; substitui o padrão. |

O importador lê a primeira aba do Excel e reconhece cabeçalhos equivalentes em português, como `NOME`, `PASTA`, `PORTA`, `BANCO`, `USUARIO` e `SENHA`. CSVs separados por vírgula, ponto e vírgula, tabulação ou `|` são aceitos. Linhas sem IP e IPs/portas repetidos são ignorados e contabilizados. As senhas podem estar em texto puro no arquivo de origem; o QueryMass não as grava em disco.

O botão **Modelo CSV** salva um exemplo preenchido com os cabeçalhos aceitos.

## Executar consultas

Selecione máquinas ou pastas, escreva uma consulta e use **Executar**. Consultas que não começam com `SELECT` e lotes com várias instruções exigem digitar `EXECUTAR` na confirmação. Os comandos rodam em paralelo em até 16 conexões; a saída mostra resultados e erros por máquina e limita a exibição a 1.000 linhas por resultado.

## Decisão técnica

O projeto usa `eframe`/`egui` para manter a interface desktop em Rust e compilar para Linux e Windows. `mysql` executa as consultas, `calamine` lê arquivos Excel, `csv` lê texto delimitado e `rfd` abre o seletor nativo de arquivos.

## Identidade visual

O QueryMass usa fundo grafite azulado com acentos em vermelho (`#FF5D70`) e azul (`#4F8CFF`). O símbolo mostra vários nós convergindo para um ponto de consulta. A referência ao Invenux se limita à organização de uma ferramenta desktop escura e à atenção à legibilidade.
