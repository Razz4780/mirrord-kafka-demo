# Setup on Kind

1. Spin up the Kafka cluster:
```sh
helm install --atomic -f kafka_values.yaml kafka-demo oci://registry-1.docker.io/bitnamicharts/kafka
```

2. Install the mirrord Operator from the [helm chart](https://github.com/metalbear-co/charts).
Remember to enable Kafka splitting with [`operator.kafkaSplitting`](https://github.com/metalbear-co/charts/blob/1d081ee9245af30d652308074b65cd2e388704f1/mirrord-operator/values.yaml#L38).

3. Build the consumer app image and load it into your local cluster:
```sh
docker build . -t kafka-demo-app && kind load docker-image kafka-demo-app
```

4. Deploy the consumer app and Kafka configuration for the operator:
```sh
kubectl apply -f cluster_setup.yaml
```

5. Build the local apps:
```sh
cargo build
```

6. To start the first Kafka splitting consumer:
```sh
mirrord exec -f .mirrord/consumer_1.json -- ./target/debug/kafka-demo consumer
```
This consumer will get only messages with header `x-user=1`.

7. To start the second Kafka splitting consumer:
```sh
mirrord exec -f .mirrord/consumer_2.json -- ./target/debug/kafka-demo consumer
```
This consumer will get only messages with header `x-user=2`.

8. To produce messages locally:
```sh
mirrord exec -f .mirrord/producer.json -- ./target/debug/kafka-demo --kafka-topic-name test --kafka-bootstrap-servers kafka-demo.default.svc.cluster.local:9092  producer --key hello --header "x-user=1"
```
The producer app accepts more args, use `./target/debug/kafka-demo producer --help` for info.
