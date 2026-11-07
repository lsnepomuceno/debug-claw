<?php

declare(strict_types=1);

namespace App;

final class Node
{
    /** @var list<Node> */
    public array $children = [];

    /** @var array<string, mixed> */
    public array $meta;

    public function __construct(
        public readonly string $name,
        public readonly ?Node $parent,
    ) {
        $this->meta = ['created' => microtime(true), 'tags' => ['a', 'b'], 'enabled' => true];
    }
}
