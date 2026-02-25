setup:
	@if [ ! -f .env ]; then \
		read -p "Enter your DATABASE_URL (e.g., postgres://user:pass@localhost:5432/db): " db_url; \
		echo "DATABASE_URL=$$db_url" > .env; \
		echo "✅ Created .env file!"; \
	else \
		echo "✅ .env file already exists."; \
	fi

up:
	docker compose up -d

down:
	docker compose down

run:
	cargo run

install: setup up
	cargo install --path .
