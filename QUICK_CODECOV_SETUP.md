# 🚀 Quick Codecov Setup (2 минуты)

## Шаг 1: Получить токен Codecov

### Вариант A: Через веб-интерфейс

1. Открой https://codecov.io/
2. Нажми **"Sign in with GitHub"**
3. В левом верхнем углу выбери организацию **`ytget`** (не `romanitalian`)
4. Нажми **"Add new repository"**
5. Найди **`ytget/ryt`** в списке
6. После добавления перейди в **Settings**
7. Скопируй **"Repository Upload Token"**

### Вариант B: Прямая ссылка

Если репозиторий уже добавлен, перейди сразу:
```
https://app.codecov.io/gh/ytget/ryt/settings
```

## Шаг 2: Добавить токен в GitHub

1. Открой https://github.com/ytget/ryt/settings/secrets/actions
2. Нажми **"New repository secret"**
3. Заполни:
   - **Name**: `CODECOV_TOKEN`
   - **Secret**: вставь токен из шага 1
4. Нажми **"Add secret"**

## Шаг 3: Проверить работу

### Автоматически (рекомендуется)
После добавления токена, следующий push автоматически отправит данные:
```bash
git commit --allow-empty -m "chore: trigger codecov with token"
git push
```

### Или через PR
Если PR #1 уже открыт:
1. Перейди на https://github.com/ytget/ryt/pull/1
2. Нажми **"Rerun all jobs"** в CI checks
3. Дождись завершения

## Шаг 4: Проверить результат

После успешного CI запуска:

1. **Codecov Dashboard**: https://app.codecov.io/gh/ytget/ryt
2. **Coverage Badge**: обновится автоматически
3. **PR Comment**: Codecov bot добавит комментарий с diff покрытия

## Что получишь

### ✅ В Codecov Dashboard
- 📊 Общее покрытие кода (%)
- 📈 График изменений во времени
- 🌳 Дерево файлов с процентами
- 🔍 Line-by-line coverage

### ✅ В Pull Requests
```
Codecov Report
Coverage: 78.5% (target: 70%)  ✅

Files changed: 5
Lines added: 120 (covered: 95, uncovered: 25)

View full report: https://codecov.io/...
```

### ✅ В README
Бэйдж автоматически покажет актуальное покрытие:
[![codecov](https://codecov.io/gh/ytget/ryt/branch/main/graph/badge.svg)]

## Troubleshooting

### "Repository not found"
- Убедись что выбрана организация `ytget`, а не `romanitalian`
- Проверь что Codecov app установлен для организации `ytget`

### "Token required"
- Нормально! Именно для этого нужно добавить токен в GitHub Secrets

### Badge показывает "unknown"
- Подожди 2-3 минуты после первого успешного upload
- Обнови страницу

## Дополнительно

### Локальная проверка покрытия

```bash
# Установить tarpaulin (один раз)
cargo install cargo-tarpaulin

# Сгенерировать HTML отчёт
cargo tarpaulin --out Html --output-dir coverage

# Открыть в браузере
open coverage/index.html  # macOS
xdg-open coverage/index.html  # Linux
```

### Настройки в codecov.yml

Текущие таргеты:
- **Project**: 70% minimum
- **Patch**: 70% for new code
- **Threshold**: 1% decrease allowed

Подробнее см. `.codecov_setup.md`

---

**Время выполнения**: ~2 минуты  
**Сложность**: 🟢 Простая

Если возникнут проблемы, см. полную инструкцию в `.codecov_setup.md`

