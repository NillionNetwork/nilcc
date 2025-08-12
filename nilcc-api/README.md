# nilcc-api

This is the public facing API that allows interacting with nilcc.

# Migrations

nilcc-api uses [typeorm](https://typeorm.io/) to manage database interactions and migrations.

Adding new migrations can be done by using `pnpm typeorm`:

```bash
pnpm typeorm migration:create migrations/AddSomethingCool
```

Migrations also need to be added to the `DataSource` setup in the [buildDataSource](src/data-source.ts) function.


