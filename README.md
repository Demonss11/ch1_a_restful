# ch1_a — минимальный прототип

Вариант на базе `proto/ch1` с **базовой архитектурой без сложенных путей**,
готовый к расширению.

## Что убрано по сравнению с `ch1`

- Content negotiation (vendor `vnd.demo.v1/v2+json`, разбор `Accept`, `Vary`/ETag-версии).
- Мультиплекс-маршрут `/builds/{id}/{kind}` (dispatch artifact/logs/diagnostics/cancel/rebuild).
- Деплойменты, идемпотентность, promote, sources, help.
- Расширенная state-machine с переходами/аффордансами в отдельных подресурсах.

## Что сохранено и добавлено (базовая архитектура)

- Модульная структура: `main` → `lib` (app/router/seed/worker) + `model` + `handlers` + `error` + `profile`.
- Разделяемое состояние `AppState { store: RwLock<Store>, build_tx: mpsc::Sender }`.
- Фоновый компилятор через `mpsc` (переход `queued → ready/failed`).
- HATEOAS-навигация: ответы содержат `_links`.
- Единый формат ошибок в стиле `problem+json`.
- **Семантический профиль** (`GET /profile`, раздел 2.2.4): машиночитаемый документ,
  фиксирующий смысл всех имён свойств, `rel`, действий, enum'ов и ошибок. Клиент
  привязывается к профилю (вместе с протоколом и медиатипом), а не к URL или схеме
  объекта — это третий агент привязки по книге.
  - Отдаётся с собственным медиатипом `application/vnd.risk-compiler.v1+profile+json`.
  - Обнаружим из корня `/_links` и из `_links` программ/билдов (`rel: "profile"`).

## Маршруты (явные, плоские)

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/` | Точка входа |
| GET | `/profile` | Семантический профиль API |
| GET | `/programs` | Список программ |
| GET | `/programs/{slug}` | Программа |
| GET | `/programs/{slug}/builds` | История сборок |
| POST | `/programs/{slug}/builds` | Создать сборку |
| GET | `/builds/{id}` | Состояние/лог сборки |

Расширение — добавлением явных маршрутов (например, `POST /builds/{id}/cancel`,
`PUT /programs/{slug}/builds/{id}/current`, `/deployments`), без catch-all `/{kind}`.

## Семантический профиль

`GET /profile` возвращает документ вида:

```json
{
  "profile": "risk-compiler.v1",
  "version": "1.0.0",
  "mediaType": "application/vnd.risk-compiler.v1+json",
  "entryPoint": "/",
  "profileUrl": "/profile",
  "resources":  { "program": {...}, "build": {...} },
  "enums":      { "buildState": { "values": ["queued","ready","failed"], "transitions": {...} } },
  "linkRelations": {
    "self":          "Canonical URI of the current resource.",
    "profile":       "This semantic profile document.",
    "programs":      "Collection of all programs.",
    "program":       "Program resource.",
    "builds":        "Collection of builds for a program.",
    "build":         "Action: create a new build for a program (POST).",
    "current-build": "Currently active build of a program."
  },
  "actions":   { "create-program": {...}, "create-build": {...}, ... },
  "errors":    { "contentType": "application/problem+json", "statuses": {...} },
  "_links":    [ {"rel": "self", ...}, {"rel": "entry", ...} ]
}
```

Профиль объявляется в `_links` каждого ответа (`rel: "profile"`), поэтому
M2M-клиент может обнаружить его на любом шаге навигации и один раз прочитать
семантику всех полей (`slug`, `title`, `n`, `state`, `artifact`) и действий
(`build` — это `POST` без тела; `current-build` появляется только при
`current != null`).

## Запуск

```bash
cargo run                # сервер на 127.0.0.1:3001
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Проверка профиля:

```bash
curl -i http://127.0.0.1:3001/profile
# HTTP/1.1 200 OK
# content-type: application/vnd.risk-compiler.v1+profile+json
# {"profile":"risk-compiler.v1",...}

curl -s http://127.0.0.1:3001/ | jq '._links[] | select(.rel=="profile")'
```