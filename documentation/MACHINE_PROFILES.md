# CARNELIAN Machine Profiles

## Overview

CARNELIAN supports two standardized machine profiles for easy deployment:

1. **Standard Profile** - Mid-range machines (16GB RAM, 4-8 cores)
2. **Performance Profile** - High-end machines (32GB+ RAM, 8+ cores, GPU)

---

## Standard Profile

**Hardware Requirements:**
- **RAM:** 16GB minimum
- **CPU:** 4-8 cores
- **GPU:** Optional (CPU-only models supported)
- **Storage:** 50GB+ SSD

**Recommended Models:**
- `deepseek-r1:7b` (7B parameters, fast inference)
- `llama3.1:8b` (8B parameters, balanced)
- `qwen2.5:7b` (7B parameters, multilingual)

**Resource Allocation:**
- Carnelian Core: 4 CPUs, 4GB RAM
- PostgreSQL: 2 CPUs, 2GB RAM
- Ollama: 4 CPUs, 8GB RAM

**Usage:**
```bash
docker-compose -f docker-compose.yml -f docker-compose.standard.yml up -d
```

**Expected Performance:**
- Skill execution: 50-100ms average
- Model inference: 2-5 seconds
- Concurrent tasks: 10-20
- Memory operations: 100-200 ops/sec

---

## Performance Profile

**Hardware Requirements:**
- **RAM:** 32GB+ recommended
- **CPU:** 8+ cores
- **GPU:** NVIDIA GPU with 11GB+ VRAM (RTX 3080/4080, A4000+)
- **Storage:** 100GB+ NVMe SSD

**Recommended Models:**
- `deepseek-r1:32b` (32B parameters, high quality)
- `qwen2.5:32b` (32B parameters, multilingual)
- `llama3.1:70b` (70B parameters, maximum quality)

**Resource Allocation:**
- Carnelian Core: 8 CPUs, 8GB RAM
- PostgreSQL: 4 CPUs, 4GB RAM (optimized settings)
- Ollama: 8 CPUs, 16GB RAM + GPU acceleration

**PostgreSQL Optimizations:**
- Max connections: 200
- Shared buffers: 1GB
- Effective cache size: 3GB

**Ollama Optimizations:**
- Parallel requests: 4
- Max loaded models: 2
- Keep alive: 24h (reduce reload overhead)

**Usage:**
```bash
docker-compose -f docker-compose.yml -f docker-compose.performance.yml up -d
```

**Expected Performance:**
- Skill execution: 20-50ms average
- Model inference: 0.5-2 seconds
- Concurrent tasks: 50-100
- Memory operations: 500-1000 ops/sec

---

## Profile Comparison

| Feature | Standard | Performance | Difference |
|---------|----------|-------------|------------|
| **RAM Required** | 16GB | 32GB+ | 2x |
| **CPU Cores** | 4-8 | 8+ | 2x |
| **GPU** | Optional | Required | - |
| **Model Size** | 7-8B | 32-70B | 4-9x |
| **Inference Speed** | 2-5s | 0.5-2s | 2.5-10x faster |
| **Concurrent Tasks** | 10-20 | 50-100 | 5x |
| **Memory Ops/sec** | 100-200 | 500-1000 | 5x |
| **Cost** | $0-500 | $1500-3000 | - |

---

## Choosing a Profile

### Use Standard Profile If:
- ✅ Budget-conscious deployment
- ✅ Development/testing environment
- ✅ Personal use or small team
- ✅ CPU-only inference acceptable
- ✅ 10-20 concurrent tasks sufficient

### Use Performance Profile If:
- ✅ Production deployment
- ✅ High-quality model responses required
- ✅ 50+ concurrent tasks needed
- ✅ GPU available
- ✅ Fast inference critical

---

## Custom Profiles

You can create custom profiles by:

1. Copy an existing profile file
2. Adjust resource limits
3. Modify environment variables
4. Test and validate

**Example Custom Profile:**
```yaml
# docker-compose.custom.yml
services:
  carnelian-core:
    deploy:
      resources:
        limits:
          cpus: "6"
          memory: 6G
```

**Usage:**
```bash
docker-compose -f docker-compose.yml -f docker-compose.custom.yml up -d
```

---

## Monitoring & Tuning

### Resource Monitoring
```bash
# Check container resource usage
docker stats

# Check PostgreSQL performance
docker exec -it carnelian-postgres psql -U carnelian -c "SELECT * FROM pg_stat_activity;"

# Check Ollama GPU usage
docker exec -it carnelian-ollama nvidia-smi
```

### Performance Tuning

**If experiencing slowness:**
1. Increase CPU/RAM limits
2. Enable GPU acceleration
3. Reduce model size
4. Optimize PostgreSQL settings

**If experiencing OOM errors:**
1. Reduce concurrent task limits
2. Increase memory reservations
3. Use smaller model
4. Enable swap (not recommended for production)

---

## Migration Between Profiles

**Upgrading Standard → Performance:**
```bash
# Stop current deployment
docker-compose down

# Start with performance profile
docker-compose -f docker-compose.yml -f docker-compose.performance.yml up -d

# Data persists automatically (PostgreSQL volume)
```

**Downgrading Performance → Standard:**
```bash
# Stop current deployment
docker-compose down

# Start with standard profile
docker-compose -f docker-compose.yml -f docker-compose.standard.yml up -d

# May need to switch to smaller model
docker exec -it carnelian-ollama ollama pull deepseek-r1:7b
```

---

## Troubleshooting

### Standard Profile Issues

**Problem:** Slow inference
- **Solution:** Reduce model size or upgrade to Performance profile

**Problem:** Out of memory
- **Solution:** Reduce concurrent tasks or increase RAM

**Problem:** High CPU usage
- **Solution:** Reduce worker count or upgrade CPU

### Performance Profile Issues

**Problem:** GPU not detected
- **Solution:** Install NVIDIA Container Toolkit, verify GPU drivers

**Problem:** Model loading slow
- **Solution:** Increase OLLAMA_KEEP_ALIVE, use NVMe storage

**Problem:** High memory usage
- **Solution:** Reduce OLLAMA_MAX_LOADED_MODELS, optimize PostgreSQL

---

## Best Practices

1. **Start with Standard** - Test and validate before upgrading
2. **Monitor Resources** - Use `docker stats` to track usage
3. **Optimize Models** - Choose appropriate model size for hardware
4. **Backup Data** - Regular PostgreSQL backups
5. **Update Regularly** - Keep Docker images current

---

## Support

For issues or questions:
- GitHub Issues: https://github.com/kordspace/carnelian/issues
- Documentation: `docs/README.md`
- Getting Started: `docs/GETTING_STARTED.md`
