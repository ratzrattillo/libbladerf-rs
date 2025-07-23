# Testing

## Run tests using the following command

```bash
cargo test -- --test-threads=1
```

## Run tests and display output

```bash
cargo test -- --nocapture --test-threads=1
```

## Run a specific test and display output

```bash
cargo test --test bladerf1_tuning -- --nocapture --test-threads=1
```
