podman run -d \
    --name ptrans-db \
    -e MARIADB_ROOT_PASSWORD=ptrans \
    -e MARIADB_DATABASE=ptrans \
    -p 3306:3306 \
    -v $(pwd)/db/setup.sql:/docker-entrypoint-initdb.d/init.sql:Z \
    mariadb:11-ubi9
