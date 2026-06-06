# Установка (Docker)

Agate спроектирован для запуска в Docker. Точка входа — бинарь **`agate-server`**
— корень композиции, который связывает контексты proxy, audit и policy. Он
настраивается смонтированным файлом **`agate.toml`** (см.
**[Конфигурацию](configuration.md)**).

## 1. Предпосылки

- Запущенный **AG-UI-агент**, перед которым ставится Agate (его URL становится
  `[proxy].agent_endpoint`).
- База **PostgreSQL** для журнала прозрачности (её URL становится
  `[audit].database_url`). Миграции выполняются автоматически при старте.

## 2. Получите образ

```bash
docker pull ghcr.io/c3equalzz/agate:latest
```

!!! note "Сборка из исходников"
    Если предпочитаете собрать локально:
    `docker build -t agate -f crates/agate-server/Dockerfile .`

## 3. Напишите `agate.toml`

Возьмите за основу [`agate.example.toml`](https://github.com/C3EQUALZz/agate/blob/main/agate.example.toml):

```toml
[proxy]
agent_endpoint = "http://your-agent:9000/run"
bind = "0.0.0.0:8080"

[audit]
database_url = "postgres://agate@db:5432/agate"  # пароль через env, ниже

[policy.tools]
mode = "allow-all"
```

## 4. Запустите

Смонтируйте файл и укажите на него `AGATE_CONFIG`; секреты передавайте
переопределениями окружения `AGATE__*`:

```bash
docker run --rm \
  -p 8080:8080 \
  -v "$PWD/agate.toml:/etc/agate/agate.toml:ro" \
  -e AGATE_CONFIG=/etc/agate/agate.toml \
  -e AGATE__AUDIT__DATABASE_URL='postgres://agate:secret@db:5432/agate' \
  ghcr.io/c3equalzz/agate:latest
```

Направьте фронтенд на `http://localhost:8080` вместо агента — Agate пересылает
каждый запрос агенту после инспекции.

## 5. Закрепите журнал прозрачности

При первом старте Agate создаёт новый журнал прозрачности и печатает его id:

```text
created transparency log 3f6c…; set AUDIT_LOG_ID=3f6c… to reuse it
```

Задайте переменную окружения `AUDIT_LOG_ID` равной этому UUID, чтобы перезапуски
дописывали в **тот же** журнал, а не создавали новый (`-e AUDIT_LOG_ID="3f6c…"`).

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
    image: ghcr.io/c3equalzz/agate:latest
    depends_on: [db]
    ports:
      - "8080:8080"
    volumes:
      - ./agate.toml:/etc/agate/agate.toml:ro
    environment:
      AGATE_CONFIG: /etc/agate/agate.toml
      AGATE__AUDIT__DATABASE_URL: "postgres://agate:agate@db:5432/agate"
      # AUDIT_LOG_ID: "…"   # задайте после первого запуска, чтобы переиспользовать журнал
```

Далее — **[Конфигурация](configuration.md)** для полного справочника по ключам.
