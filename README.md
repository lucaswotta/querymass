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

A interface inicial divide o aplicativo entre a lista de máquinas, o editor SQL e a área de saída. O cadastro de conexões e a execução de consultas ainda não estão implementados.

## Decisão técnica

O projeto usa `eframe`/`egui` para manter a interface desktop em Rust e compilar para Linux e Windows.

## Identidade visual

O QueryMass usa fundo grafite azulado com acentos em vermelho (`#FF5D70`) e azul (`#4F8CFF`). O símbolo mostra vários nós convergindo para um ponto de consulta. A referência ao Invenux se limita à organização de uma ferramenta desktop escura e à atenção à legibilidade.
