for PORT in 8080 8081 8082 8083 8084
do
  cargo run --bin node $PORT localhost
done