# Codecov Troubleshooting - ytget/ryt

## Проблема: Codecov не находит репозиторий

### ✅ Что уже настроено:
- Codecov app установлен для организации `ytget`
- Репозиторий `ytget/ryt` добавлен в доступ
- Права: Read/Write для checks, commit statuses, pull requests

### 🔧 Решения:

#### 1. **Прямая ссылка на репозиторий**
Перейдите напрямую по ссылке:
```
https://app.codecov.io/gh/ytget/ryt
```

#### 2. **Через GitHub репозиторий**
1. Откройте https://github.com/ytget/ryt
2. Нажмите на вкладку "Actions"
3. Запустите любой workflow (или создайте коммит)
4. После завершения CI должен отправить данные в Codecov

#### 3. **Принудительный запуск CI**
```bash
# Сделать коммит чтобы запустить CI
git add .
git commit -m "feat: trigger codecov setup"
git push
```

#### 4. **Проверка в Codecov**
После push проверьте:
- https://app.codecov.io/gh/ytget/ryt
- Если репозиторий не найден - подождите 2-3 минуты
- Попробуйте обновить страницу

#### 5. **Альтернативный способ добавления**
1. Перейдите на https://codecov.io/
2. Нажмите "Sign in with GitHub"
3. В поиске введите: `ytget/ryt`
4. Если не найден - нажмите "Can't find your repo? Resync"

### 🔍 Проверка настроек

#### GitHub App настройки:
- ✅ Установлен для `ytget`
- ✅ Доступ к `ytget/ryt`
- ✅ Права на checks и PR

#### CI Workflow:
- ✅ `.github/workflows/ci.yml` создан
- ✅ Coverage job настроен
- ✅ tarpaulin и codecov-action добавлены

### 📝 Следующие шаги:

1. **Сделать коммит** для запуска CI
2. **Дождаться** завершения GitHub Actions
3. **Проверить** https://app.codecov.io/gh/ytget/ryt
4. **Добавить токен** если репозиторий появится

### ⚠️ Если ничего не помогает:

1. **Удалить и переустановить** Codecov app:
   - GitHub Settings → Applications → Codecov → Uninstall
   - Повторно установить для `ytget`

2. **Проверить права** в организации `ytget`:
   - Убедиться что у вас admin права
   - Проверить что репозиторий `ryt` существует

3. **Альтернатива - личный токен**:
   - Создать токен на https://codecov.io/account/token
   - Добавить в GitHub Secrets как `CODECOV_TOKEN`
