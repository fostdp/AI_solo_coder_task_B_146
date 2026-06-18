FROM python:3.11-slim-bookworm

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY hunyi_simulator.py .

RUN chmod +x hunyi_simulator.py

HEALTHCHECK --interval=30s --timeout=5s \
    CMD python -c "import sys; sys.exit(0)" || exit 1

ENTRYPOINT ["python", "hunyi_simulator.py"]
CMD ["--mqtt", "mqtt:1883", "--fast"]
