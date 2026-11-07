<?php

declare(strict_types=1);

require __DIR__ . '/../src/bootstrap.php';

use App\Graph;
use App\OrderService;
use App\Queries;

$path = parse_url($_SERVER['REQUEST_URI'] ?? '/', PHP_URL_PATH);
$method = $_SERVER['REQUEST_METHOD'] ?? 'GET';

header('Content-Type: application/json');

try {
    $result = match ([$method, $path]) {
        ['GET', '/'] => ['routes' => [
            'GET /loop?n=100',
            'GET /graph?depth=6&width=3',
            'GET /queries?n=100',
            'GET /warning',
            'POST /orders customer_id=N',
        ]],
        ['GET', '/loop'] => loop((int) ($_GET['n'] ?? 100)),
        ['GET', '/graph'] => Graph::build((int) ($_GET['depth'] ?? 6), (int) ($_GET['width'] ?? 3))->summary(),
        ['GET', '/queries'] => (new Queries())->run((int) ($_GET['n'] ?? 100)),
        ['GET', '/warning'] => warning(),
        ['POST', '/orders'] => (new OrderService())->place((int) ($_POST['customer_id'] ?? 1)),
        default => notFound(),
    };
    echo json_encode($result, JSON_PRETTY_PRINT), "\n";
} catch (Throwable $e) {
    http_response_code(500);
    echo json_encode(['error' => $e::class, 'message' => $e->getMessage()]), "\n";
}

/** A loop with a value that changes on every iteration, for logpoints. */
function loop(int $n): array
{
    $sum = 0;
    for ($i = 0; $i < $n; $i++) {
        $item = ['index' => $i, 'square' => $i * $i];
        $sum += $item['square'];
    }

    return ['iterations' => $n, 'sum' => $sum];
}

/** Raises an E_WARNING, for breakpoints on PHP errors. */
function warning(): array
{
    $settings = [];
    $missing = @$settings['missing'];
    $value = $settings['missing'];

    return ['value' => $value ?? $missing];
}

function notFound(): array
{
    http_response_code(404);

    return ['error' => 'not found'];
}
