
target/debug/qualinvest_server -c qualinvest_test.toml  >/tmp/qlserver.log &
ql_pid=$!
echo "pid of qualinvest_server is $ql_pid"

echo "start docker"
sudo docker start qlSelDocker
sleep 5

echo "start tests"
mkdir -p /tmp/gui_test_results
target/debug/gui_tests

echo "stop docker"
sudo docker stop qlSelDocker

echo "stop qualinvest_server"
kill $ql_pid

