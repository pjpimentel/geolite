# decisions

## **2026-08-16** (dissolução do `src/pbf`)

1. `src/pbf` **deixou de existir**. ele nasceu na fatia do `osm_node` para tirar o formato de fio de
   dentro de um estágio, e resolveu aquilo — mas ficou como módulo de topo que não é tabela, não é
   estágio e não tem dono. tanto que o `mod.rs` do `osm_pbf_file` precisava de uma linha só para
   dizer "não confundir com `crate::pbf`".
2. **`osm_pbf_file` é o domínio que sabe interpretar o arquivo** — qual arquivo é, de onde veio, como
   está disposto em bytes, o que o cabeçalho diz e como esses bytes viram mensagens. `message` e
   `compression` eram a peça que faltava desse mesmo conceito, não uma segunda coisa: a pasta já
   guardava o catálogo, a tabela de faixas, o cliente http e o download.
3. **a regra que isso fixa**: formato compartilhado sem dono fica fora do domínio; formato *de
   alguém* mora com esse alguém. `database/jsonb.rs` continua fora porque é o formato de
   armazenamento do sqlite, usado por quem escreve payload e de ninguém em particular.
4. as treze mensagens continuam num arquivo só porque **se aninham** — `primitive_group_msg` tem
   `node_msg`, `way_msg` e `relation_msg` como campos. distribuí-las pelos domínios de elemento faria
   o formato de fio depender do domínio, que é a direção invertida, e tiraria o que torna uma
   transcrição revisável: ler os números de tag lado a lado contra a spec.
5. `tag_policy` **não era do arquivo** e foi para `osm_tag::policy`. duas evidências: `osm_pbf_file`
   nunca a usava, e quem a preenche são as flags `--tags-include`/`--tags-ignore` no cli. o formato
   pbf não liga para quais tags você guarda. ela estava em `src/pbf` por inércia desde a fatia do
   `osm_node`. o paralelo é `house_number::policy`: uma política mora com o vocabulário que ela
   filtra.
6. a direção `osm_node → osm_pbf_file` que isso cria se lê como *"um nó é decodificado a partir de um
   arquivo pbf"*, e não fecha ciclo — `osm_pbf_file` não importa nenhum domínio de elemento.

## **2026-08-16** (comentários)

1. **comentário só para workaround ou risco real de performance.** o resto — o que uma coisa é, o
   que uma função faz, por que um tipo existe — é carregado por nome e tipo; comentário que repete o
   código é excesso. virou a regra 11 do `CLAUDE.md`.
2. das **554 linhas** de comentário em `src/domain`, sobraram **15, em seis blocos**: o artefato de
   paralelismo entre pares do mesmo nível no `resolver` e a reaplicação que o corrige; os dois
   fallbacks de `conn.path()` devolvendo `Some("")` para `:memory:`; e a nota, nos três row structs
   de elemento, de que codificar no insert em vez de nas threads decodificadoras serializaria o
   pipeline.
3. a explicação de módulo e de pasta passou a ser responsabilidade do `readme.md`, não de um bloco
   no topo do arquivo. foi por isso que o doc de topo — que a passagem anterior tinha acabado de
   padronizar em 40 arquivos — saiu inteiro.
4. **o comentário `--` dentro do `CREATE TABLE` de `admin_levels` ficou.** ele não é comentário
   rust: está dentro do literal e o sqlite o guarda em `sqlite_master`, então removê-lo mudaria o
   texto do schema gravado em disco.
5. o hash do binário release muda, e isso **não** invalida a mudança: `.expect()` grava `file:line`
   no binário, e apagar 554 linhas desloca todas. a equivalência foi provada pelo rebuild completo
   de andorra e pelo caminho de leitura, não pelo hash.

## **2026-08-16** (coerência do domínio)

1. **onde mora o sql**: o bloco de ddl agrupado no topo, todo outro const imediatamente acima da
   única função que o usa. a medição que decidiu isso: **68 dos 68 consts têm um consumidor só**, e
   nenhum teste referencia um const de produção. escopo de módulo declara "qualquer função aqui pode
   usar", o que é falso em 68 de 68.
2. **o ddl é a exceção, e por um motivo operacional, não estético.** ele não é uma consulta, é o
   schema — e é a âncora de `sed -n '/^const SQL_CREATE: /,/^";\$/p'`, o comando que provou byte a
   byte, em sete migrações seguidas, que nenhuma base existente precisava de rebuild. enterrá-lo
   dentro de `fn create_table` custaria essa verificação.
3. considerou-se pôr a consulta **dentro** do corpo da função — escopo honesto, nomes curtos
   (`SQL_LOAD_METADATA_BY_IDS_PREFIX` viraria `SQL`), e as consultas montadas com `format!` deixariam
   de ficar partidas. não foi feito: são 49 blocos de sql reindentados à mão, e erro de sql neste
   projeto falha em silêncio. o ganho não paga o modo de falha.
4. `admin_level/scale.test.rs` era um arquivo de **zero bytes** com um `mod tests;` apontando para
   ele — o `level` não tinha nenhum teste e o arquivo fingia que tinha. o arquivo e a declaração
   saíram; a cobertura virou tarefa, com o motivo por extenso.
5. o índice de blob chunks foi renomeado para carregar o nome da tabela, e o `create_indexes` ganhou
   um `DROP INDEX IF EXISTS` do nome antigo. renomear índice sem retirar o velho deixaria as duas
   cópias numa base existente, pagas em toda escrita.

## **2026-08-16** (osm_tag)

1. `osm_tag` é a **primeira pasta do domínio que não é uma tabela**. a regra "cada domínio toma o
   nome da sua tabela" continua valendo para tudo que é linha; isto é vocabulário compartilhado, como
   `admin_level::scale` é escala e não linha. fica sob `domain` e não ao lado de `pbf` porque uma tag
   carrega significado — `pbf` é formato, isto é o que o formato está dizendo.
2. **duas fronteiras, escritas antes de qualquer código.** `osm_tag` é dono da chave e da forma do
   valor; não é dono do significado da combinação. "way com `highway` e sem `building` é uma rua"
   continua sendo `way_filter` (achado #12 da spec), e quais tags carregam número continua sendo
   política de `house_number`. e ele nomeia o vocabulário **interpretado**, não o armazenado:
   `pbf::tag_policy` segue stringly-typed de propósito, porque o pipeline grava chave que ninguém
   aqui conhece.
3. **a premissa que vendeu a fatia estava parcialmente errada, e o registro fica.** eu havia afirmado
   que oito das dezesseis chaves precisavam de aspas no caminho json sob pena de NULL silencioso.
   medindo: `:` e `-` funcionam sem aspas — nenhuma chave em uso hoje precisa delas. quem quebra em
   silêncio é `.` (lido como caminho aninhado) e `[` (lido como índice de array), e `is_valid_key` já
   rejeita os dois. o perigo é latente, não corrente. o que sustenta a fatia é o resto: a regra de
   `NULLIF(UPPER(TRIM(COALESCE(...))))` duplicada byte a byte entre os repositórios de way e relation,
   a segurança contra erro de digitação em chave, e os conjuntos fechados de valor.
4. `is_valid_key` veio de `parse_name_priority`, no cli. o que é uma chave legal e como uma chave se
   escreve são a mesma preocupação, e os dois caracteres que a validação proíbe são exatamente os dois
   que um caminho sem aspas leria errado.
5. `database/name_select.rs` virou `osm_tag::select::coalesce_of`. seus quatro chamadores eram todos
   do domínio, então a mudança troca `domain → database` por `domain → domain`. `src/database/` ficou
   com o ciclo de vida da conexão, o writer jsonb e o merge.
6. os `const SQL_` que **não variam** ficaram com o literal escrito à mão (regra 3 do `CLAUDE.md`).
   `osm_tag` entra onde a expressão é composta em tempo de execução. compor um const estático
   trocaria uma regra da casa por outra sem ganho.

## **2026-08-16** (admin_level_hierarchy)

1. a hierarquia virou **fatia própria** e não colunas do `admin_level`, apesar de a tabela ser 1:1 e
   cascatear com ele. o critério que manteve o rtree dentro do `admin_level` — aceleração pura,
   recomputável, com um leitor só — falha aqui nas duas metades: `user_friendly_name` é **conteúdo
   renderizado**, com formatação regional, e há três leitores independentes. o que decide é que o
   documento do tantivy é um por **linha de hierarchy**, não por área: a linha da hierarquia, e não a
   área, é a unidade de busca.
2. `pending_total` e `pending_street_ids` saíram do `admin_level/repository.rs`. as duas faziam
   `JOIN admin_levels_hierarchy` a partir de quem não é dono da tabela, e as duas perguntam *"quais
   linhas eu ainda devo?"* — pergunta da hierarquia. o repositório do `admin_level` encolheu e parou
   de citar tabela alheia.
3. a regra do rótulo saiu de três pontos enterrados num arquivo de 496 linhas e virou `label`, com
   uma definição só. **não** foi fundida com `query::render_friendly_name`: os dois constroem coisas
   diferentes — um compõe a cadeia na indexação a partir do rótulo já pronto do pai, o outro renderiza
   um template sobre as áreas resolvidas na consulta. a tarefa `place_label` segue aberta, e `label`
   é onde ela vai aterrissar.
4. `index/coordinates.rs` fundiu com `admin_level/spatial_index.rs` em vez de virar arquivo irmão. em
   `osm_pbf_file` a tabela e o passe que a preenche ficaram separados (`blob_index`/`blob_scanner`)
   porque o scanner varre um **arquivo**; aqui as duas metades são sobre a mesma tabela.
5. a árvore de navegação (`roots`/`children_of`/`path_to`) ficou **de fora**. medindo na base real,
   expandir um nó hoje é `SCAN` de 3.088.808 linhas — 0,82 s por clique — porque a cadeia é
   desnormalizada apontando para cima e a tabela não tem índice nenhum além da pk. o conserto é um
   índice sobre expressão (`JSON_EXTRACT(ancestor_ids, '$[0]')`), que o planner usa como covering
   index inclusive para as raízes, **sem tocar no schema nem exigir rebuild**. é passagem própria.

## **2026-08-15** (osm_pbf_file)

1. a fatia nasceu **sem `entity`**, e é a primeira. a linha de `osm_pbf_files` é um livro-razão
   escrito em grupos de colunas — geofabrik, download, header, contagens — e lido só por
   `file_path`, `geofabrik_url` e `id`. o struct que leria a linha inteira, a consulta por trás dele
   e o `map_row` estavam os três mortos atrás de `#[allow(dead_code)]`, sem um único chamador: foram
   apagados em vez de mudados de lugar, como já se fez com as impls `ToSql`/`FromSql` das fatias de
   elemento. o ddl no `repository` é a forma.
2. `osm_pbf_blob_chunks` entrou **dentro** da pasta, como `blob_index`, e não como fatia própria. é
   o mesmo arranjo de `admin_level/spatial_index.rs`: uma segunda tabela subordinada à primeira. uma
   faixa de bytes é faixa **de um arquivo** e não significa nada sem ele — `file_id` é seu dono, e a
   varredura que a produz é uma leitura do arquivo.
3. `catalog::geofabrik` e `catalog::resolve_geofabrik_url` passaram a receber `&Connection` em vez
   de um caminho. como estavam, abriam a conexão por dentro e teriam recriado o ciclo `database ↔
   domain` fechado na fatia anterior. **um domínio nunca é dono do ciclo de vida da conexão** — é a
   única coisa que `database/mod.rs` ainda guarda. de quebra, `download` deixou de abrir duas.
4. `ureq`, `md5`, `serde` e `serde_json` entraram em `src/domain` sem cerimônia: a fatia vertical diz
   que cada domínio é dono do próprio i/o, e buscar o catálogo é o i/o deste do mesmo jeito que
   sqlite é o dos outros.
5. as três mensagens protobuf que tinham ficado para trás quando `src/pbf` nasceu —
   `blob_header_msg`, `header_block_msg` e `header_bbox_msg` — foram para `pbf/message.rs`. era o
   que faltava para nenhum domínio depender de um estágio: `src/extract/` ficou só com os três
   estágios que sobraram.

## **2026-08-15** (osm_relation)

1. com as três fatias de elemento osm prontas, o **ciclo 1** da `spec.md` §2.3 (`database` ↔ `extract`) deixou de existir: as impls `ToSql`/`FromSql` que o formavam estavam mortas nos três casos e foram apagadas em vez de mudadas de lugar.
2. `extract/osm_data/` ficou só com o pipeline — filas, threads, backpressure e o writer. `element_payload.rs`, que era um débito assumido na fatia de node, desapareceu.
3. mover as três tabelas para o domínio criou um ciclo novo entre `database` e `domain`: as repositories precisavam de `build_name_select`, que morava no mesmo arquivo que cria as tabelas. `build_name_select` virou `database/name_select.rs`, um arquivo folha — junto com `database/jsonb.rs`, é tudo que o domínio importa de `database`. só `database/mod.rs` depende do domínio, e é o que se espera de quem compõe o schema.

## **2026-08-15** (osm_way)

1. o `enum filters` — treze predicados sobre as tags de uma way, cada um com seu fragmento de sql — foi separado em dois: o significado (`way_filter`) mora no domínio, e a tradução para sql fica no repositório. é o que permite um preset dizer `exclude_building` sem saber que existe uma coluna `payload` com json dentro. era o achado #12 da `spec.md`.
2. `remaining_ids_by_tags` passou a receber um `level` em vez de um `u8`, o que fez o domínio de way depender do de admin_level. domínio dependendo de domínio é a direção certa.
3. as impls `ToSql`/`FromSql` de `osm_way` também estavam mortas, pelo mesmo motivo das de node, e foram apagadas.

## **2026-08-15** (osm_node e o formato pbf)

1. o que é compartilhado entre os elementos osm tem **duas naturezas e dois lugares**: o formato pbf de leitura (mensagens protobuf, descompressão do blob, política de tags) foi para `src/pbf/`, um módulo de topo ao lado de `database`; o writer jsonb de escrita foi para `src/database/jsonb.rs`, a mesma prateleira de `build_name_select`. juntá-los num módulo só seria misturar o formato de entrada com o de saída.
2. os encoders **por elemento** não são genéricos: cada um pertence ao seu elemento. o de node foi para o domínio; os de way e relation ficam num arquivo do estágio até as fatias deles chegarem.
3. `osm_node_row` é público, ao contrário do row struct de `house_number`. a extração constrói linhas em várias threads decodificadoras ao mesmo tempo e dimensiona o buffer de escrita pelo payload já codificado; receber um `osm_node` no insert moveria a codificação para a única thread escritora. `osm_node_row::encode` continua sendo a única forma de construir uma linha, e ela recebe um node.
4. um domínio nunca depende de um estágio. foi essa regra que obrigou o formato pbf a sair de `extract/osm_data/`, e é ela que decide onde qualquer peça compartilhada mora daqui pra frente.

## **2026-08-15** (organização do domínio)

1. o domínio é organizado em **fatias verticais**: uma pasta por conceito, cada uma dona do que aquele conceito precisa — modelo, política, serviços e a própria persistência. os módulos técnicos (`extract`, `index`, `query`, `optimize`, `database`) são esvaziados nessas pastas e desaparecem conforme cada fatia chega.
2. o que fica de fora é o que não pertence a conceito nenhum: o ciclo de vida da conexão sqlite (`database/mod.rs`), o cli e o servidor http. `database/` encolhe muito mas não desaparece.
3. **cada domínio toma o nome da sua tabela.** `admin_level` é a tabela `admin_levels`, e a escala administrativa é um enum `level` dentro dela. não existe `admin_area` — foi um nome inventado para uma entidade que já tinha nome.
4. não há shared kernel. a identidade de uma linha (`admin_level_id`) mora na pasta da entidade a que pertence, não numa pasta de vocabulário compartilhado.
5. `bounding_box` desceu para `admin_level/geometry.rs`: é o que o rtree indexa, e mantê-lo em `query` obrigaria a persistência a depender da consulta. isso fecha o ciclo 2 descrito em `spec.md` §2.3.

## **2026-08-15** (admin_level)

1. `admin_level` is a closed set: only the named levels (1..10, 12, 14, 30) exist. an unnamed level cannot be extracted, so accepting one as a filter meant answering an empty list without saying why. it is now refused at the edge that reads it — the cli flags and the http query string.
2. because the set is closed, the type is an enum rather than a newtype over `u8`: `name()` becomes exhaustive and needs no "unknown" fallback, and an invalid level is unrepresentable rather than merely unconstructed.
3. ordering is implemented explicitly by level value instead of derived. a derived `Ord` on an enum follows declaration order, which would silently change the hierarchy the day a variant moved.
4. the query dto `admin_level` was renamed to `query_admin_level`: it is an area resolved at a level, not the level itself. the json payload is unchanged; the `/openapi.json` component name is not.
5. reading a level the scale does not name drops the row with a warning instead of failing the read — the same posture `admin_geometry::column_result` already takes with a blob it cannot parse. only a database written by another version can hold one.

## **2026-08-15** (house_number)

1. start the ddd refactor by the `house_number` domain (`src/domain/house_number`), because the concept was split across four places with two different rules in two languages — and that split was the open colombian bug, not just an aesthetic problem.
2. the `house_number` value object keeps two forms: `stored_form`, byte-identical to what the sql rule used to write, and `comparison_key`, the aggressively canonical form equality reads. this is only possible because the comparison between a typed number and the stored ones already happened in rust, never in sql — so recognising the colombian nomenclature required **no rebuild** of existing databases.
3. which written forms are recognised is per-region policy, not a global rule: `82-52` is a compound number in colombia and a range in brazil, and nothing in the string tells them apart. only the colombian preset enables `compound` and the `#` prefix.
4. the normalisation of `addr:housenumber` moved out of `SQL_LOAD_ALL_CANDIDATES` into the domain. the sql now returns the raw tag value; the drop list and the blank check run in rust, over every row the query returns.
5. `geo` is allowed inside `src/domain`; `geozero` is not — the first is a geometry model, the second is wkb/wkt serialisation and therefore infrastructure.

## **2026-08-02**

1. add more unit tests to cover more than 90% before start refactoring some parts.

## **2026-06-28** (before public release)

1. rewrite from deno/node to rust due limitations on memory control and multi threading.
2. pre-process admin level hierarchies to optimize text search.
3. drop the usage of spatiallite (sqlite extension) to decouple from sqlite.
4. split sqlite database between osm data and data to speed up optimization.
5. drop sqlite fts (trigam or unicode61) in flavor of tantivy.
