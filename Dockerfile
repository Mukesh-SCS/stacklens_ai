FROM python:3.12-slim
WORKDIR /app
COPY stacklens_ai /app/stacklens_ai
ENV HOST=0.0.0.0 PORT=8000 PYTHONDONTWRITEBYTECODE=1
EXPOSE 8000
CMD ["python", "-m", "stacklens_ai.server"]
