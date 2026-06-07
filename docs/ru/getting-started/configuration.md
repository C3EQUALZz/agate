# Конфигурация

`agate-server` (точка входа в Docker) настраивается одним файлом **`agate.toml`**
со слоем переопределений из окружения. Смонтируйте файл в контейнер и укажите на
него `AGATE_CONFIG`. Готовый шаблон —
[`agate.example.toml`](https://github.com/C3EQUALZz/agate/blob/main/agate.example.toml).

## Источники и приоритет

От низшего к высшему — каждый следующий слой переопределяет предыдущий:

1. **Встроенные значения по умолчанию.**
2. **`agate.toml`** — путь из `AGATE_CONFIG` (по умолчанию `/etc/agate/agate.toml`).
   Отсутствие файла допустимо — применяются значения по умолчанию.
3. **Окружение** — `AGATE__SECTION__KEY` (в верхнем регистре, `__` между уровнями).
   Секреты предпочтительно задавать через окружение.

То есть `[audit].database_url` переопределяется через `AGATE__AUDIT__DATABASE_URL`.

```bash
docker run --rm \
  -v "$PWD/agate.toml:/etc/agate/agate.toml:ro" \
  -e AGATE_CONFIG=/etc/agate/agate.toml \
  -e AGATE__AUDIT__DATABASE_URL='postgres://agate:secret@db:5432/agate' \
  ghcr.io/c3equalzz/agate
```

Отсутствие **обязательного** значения (`proxy.agent_endpoint`,
`audit.database_url`) прерывает запуск — отказ при неверной конфигурации вместо
работы в деградированном режиме.

## `[proxy]`

| Ключ | Обязателен | По умолчанию | Значение |
| --- | --- | --- | --- |
| `agent_endpoint` | **да** | — | URL вышестоящего AG-UI-агента, которому Agate пересылает проинспектированный трафик. |
| `bind` | нет | `0.0.0.0:8080` | Адрес/порт, который Agate слушает для входящего AG-UI-трафика. |
| `connect_timeout_secs` | нет | `5` | Быстрый таймаут подключения к вышестоящему агенту. |
| `read_timeout_secs` | нет | `60` | Таймаут простоя между чанками SSE-ответа. **Не** общий дедлайн — здоровый поток работает дальше. |
| `max_body_bytes` | нет | `1048576` | Максимальный размер тела запроса (1 МиБ). Слишком большие запросы получают `413`. |
| `api_key` | нет | — | Если задан — требуется в заголовке `X-API-Key` (иначе `401`). Пусто/отсутствует — прокси открыт. Секрет лучше через `AGATE__PROXY__API_KEY`. |

!!! note "Пробы liveness и readiness"
    `/healthz` (liveness) возвращает `200`, пока процесс жив. `/readyz`
    (readiness) возвращает `200` только когда база журнала прозрачности
    доступна, иначе `503` — направьте на него readiness-пробу оркестратора,
    чтобы трафик удерживался, пока Agate не сможет записывать. Обе пробы
    обходят проверки API-ключа и размера тела.

## `[audit]`

| Ключ | Обязателен | По умолчанию | Значение |
| --- | --- | --- | --- |
| `database_url` | **да** | — | Строка подключения PostgreSQL для журнала прозрачности (дерево Меркла). Миграции выполняются при старте. Пароль лучше задавать через `AGATE__AUDIT__DATABASE_URL`. |

Журнал, в который добавляются записи, закрепляется переменной окружения
**`AUDIT_LOG_ID`** (UUID). Если она не задана, при старте создаётся новый журнал и
его id печатается, чтобы закрепить его при следующем запуске.

## `[policy.tools]` и `[policy]`

Все ключи политики необязательны. Если ничего не задано, **все инструменты
разрешены и ничего не редактируется**.

| Ключ | Формат | Значение |
| --- | --- | --- |
| `[policy.tools].mode` | `allow-all` \| `allowlist` \| `denylist` | Как авторизуются вызовы инструментов. По умолчанию `allow-all`. |
| `[policy.tools].names` | массив имён инструментов | Инструменты, управляемые `mode` (игнорируются при `allow-all`). |
| `[policy].redact` | массив литеральных маркеров | Подстроки, маскируемые (без учёта регистра) в исходящем тексте до его доставки клиенту. |
| `[policy].fail_mode` | `open` \| `closed` | Что делать при таймауте решения политики: переслать (`open`) или заблокировать (`closed`). По умолчанию `closed` (безопасность важнее доступности). |
| `[policy].decision_timeout_ms` | целое (мс) | Дедлайн одного решения политики. По умолчанию `5000`; должен быть > 0. |

!!! warning "Неверная политика прерывает запуск"
    Пустое или некорректное имя инструмента, либо пустой паттерн редактирования,
    **прерывает запуск** — опечатка не должна молча ослаблять контроль.

## `[observability.logging]`

| Ключ | По умолчанию | Значение |
| --- | --- | --- |
| `enabled` | `true` | Устанавливать ли подписчик логов вообще; `false` отключает логи. |
| `format` | `pretty` | `pretty` (консоль) или `json` (по объекту на строку, для сборщиков логов). |
| `level` | `info` | Директива фильтра (например, `agate_proxy=debug,info`). `RUST_LOG` переопределяет её, если задана. |

На `info` видны события жизненного цикла: старт, каждый проксированный прогон,
запреты и редакции политики, создание журнала прозрачности. Поднимите до `debug`
(например, `level = "agate_proxy=debug,info"`) для детализации по каждому событию
(каждое переданное/буферизованное событие, каждая добавленная запись аудита).

## `[observability.metrics]`

Эндпоинт для Prometheus на **отдельном порту**, не на публичном data-plane
порту (скрейпится из внутренней сети).

| Ключ | По умолчанию | Значение |
| --- | --- | --- |
| `enabled` | `false` | Устанавливать ли recorder + экспортёр метрик. Если выключено — метрики no-op. |
| `exporter` | `prometheus` | `prometheus` (эндпоинт `/metrics`) или `none`. |
| `bind` | `0.0.0.0:9090` | Адрес, который слушает эндпоинт `/metrics`. |

Экспортируемые метрики:

- `agate_runs_total` — проксированных прогонов.
- `agate_events_inspected_total{outcome="forward|buffer|transform|deny|terminate"}` — проинспектированные события по исходу (разбивка по вердиктам).
- `agate_upstream_errors_total` — ошибки запроса/потока к вышестоящему агенту.
- `agate_audit_records_appended_total` / `agate_audit_records_dropped_total` — записи в журнал прозрачности против дропов (ненулевой drop-rate = аудит не успевает, ставьте алерт).

Готовый стек Prometheus + Grafana с преднастроенным дашбордом — в
[`deploy/observability/`](https://github.com/C3EQUALZz/agate/tree/main/deploy/observability).

## `[observability.tracing]`

Экспорт трейсов по OTLP — третий столп наблюдаемости наряду с логами и метриками.
Когда выключено, спаны всё равно создаются (и видны в логах), но не экспортируются.

| Ключ | По умолчанию | Значение |
| --- | --- | --- |
| `enabled` | `false` | Экспортировать спаны в OTLP-коллектор по gRPC. |
| `endpoint` | `http://localhost:4317` | OTLP gRPC-эндпоинт коллектора. |
| `service_name` | `agate-server` | `service.name` в экспортируемых спанах. |

Спаны покрывают путь запроса от края до края:

- `proxy_run` — по одному на каждый проксированный прогон на data-plane.
- `audit.request` — по одному на каждую отправленную команду/запрос аудита.
  `TracingBehavior` оборачивает весь конвейер медиатора (самым внешним звеном,
  поверх behaviour'ов метрик и транзакции), поэтому каждый use case трассируется
  единообразно — новые use case'ы получают спан автоматически.
- `db.log.load` / `db.log.save` / `db.proof.inclusion` / `db.proof.consistency`
  — по одному на каждый SQL-оператор, вложенному в спан `audit.request`,
  который его инициировал.

Спаны сбрасываются при graceful shutdown. Укажите `endpoint` на OpenTelemetry
Collector (или любой OTLP/gRPC-бэкенд), чтобы собирать трейсы по каждому прогону.

## Полный пример

```toml
[proxy]
agent_endpoint = "http://agent:8000/run"
bind = "0.0.0.0:8080"
connect_timeout_secs = 5
read_timeout_secs = 60
max_body_bytes = 1048576
# api_key = "change-me"   # секрет лучше через AGATE__PROXY__API_KEY

[audit]
# Пароль лучше задавать через AGATE__AUDIT__DATABASE_URL.
database_url = "postgres://agate@postgres:5432/agate"

[policy.tools]
mode = "allowlist"
names = ["search", "fetch"]

[policy]
redact = ["sk-", "AKIA"]
fail_mode = "closed"
decision_timeout_ms = 5000

[observability.logging]
enabled = true
format = "pretty"
level = "info"

[observability.metrics]
enabled = true
exporter = "prometheus"
bind = "0.0.0.0:9090"

[observability.tracing]
enabled = false
endpoint = "http://localhost:4317"
service_name = "agate-server"
```
