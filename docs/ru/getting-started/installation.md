# Установка (Docker)

Agate спроектирован для запуска в Docker. Точка входа — бинарь **`agate-server`**
— корень композиции, который связывает контексты proxy, audit и policy.

!!! warning "Образ ещё не опубликован"
    Готовый образ пока не опубликован в реестре. До этого момента собирайте
    образ из репозитория. Имя `agate-server` ниже — это заглушка для того тега,
    которым вы пометите свою сборку.

## 1. Предварительные требования

- Запущенный **AG-UI-агент**, перед которым размещается Agate (его URL
  становится `AGENT_ENDPOINT`).
- База данных **PostgreSQL** для журнала прозрачности (её URL становится
  `DATABASE_URL`). Миграции выполняются автоматически при запуске.

## 2. Соберите образ

```bash
# из корня репозитория
docker build -t agate-server -f crates/agate-server/Dockerfile .
```

!!! note
    Если крейт ещё не поставляет `Dockerfile`, соберите бинарь командой
    `cargo build --release -p agate-server` и запустите его напрямую с теми же
    переменными окружения, что описаны ниже. `Dockerfile` в планах.

## 3. Запустите

```bash
docker run --rm \
  -p 8080:8080 \
  -e AGENT_ENDPOINT="http://your-agent:9000" \
  -e DATABASE_URL="postgres://agate:agate@db:5432/agate" \
  -e BIND_ADDR="0.0.0.0:8080" \
  agate-server
```

Укажите ваш фронтенд на `http://localhost:8080` вместо агента. Agate
перенаправляет каждый запрос на `AGENT_ENDPOINT` после инспекции.

## 4. Закрепите журнал прозрачности

При первом запуске Agate создаёт новый журнал прозрачности и выводит его id:

```text
created transparency log 3f6c…; set AUDIT_LOG_ID=3f6c… to reuse it
```

Задайте `AUDIT_LOG_ID` равным этому UUID, чтобы перезапуски добавляли записи в
**тот же** журнал, а не начинали новый:

```bash
docker run --rm \
  -e AUDIT_LOG_ID="3f6c0b1e-…" \
  ... \
  agate-server
```

## 5. Примените политику (необязательно)

По умолчанию Agate разрешает все инструменты и ничего не редактирует. Ужесточите
это переменными `POLICY_*` (см. **[Конфигурацию](configuration.md)**):

```bash
docker run --rm \
  -e POLICY_TOOL_ALLOWLIST="search,read_file" \
  -e POLICY_REDACT_PATTERNS="sk-,AKIA" \
  ... \
  agate-server
```

## Пример: Docker Compose

```yaml
services:
  db:
    image: postgres:17
    environment:
      POSTGRES_USER: agate
      POSTGRES_PASSWORD: agate
      POSTGRES_DB: agate

  agate:
    image: agate-server # пока собирается локально
    depends_on: [db]
    ports:
      - "8080:8080"
    environment:
      AGENT_ENDPOINT: "http://your-agent:9000"
      DATABASE_URL: "postgres://agate:agate@db:5432/agate"
      BIND_ADDR: "0.0.0.0:8080"
      # AUDIT_LOG_ID: "…"           # задайте после первого запуска для повторного использования журнала
      # POLICY_TOOL_ALLOWLIST: "…"  # см. Конфигурацию
```

Перейдите к **[Конфигурации](configuration.md)** за полным справочником по
переменным.
