<?php

declare(strict_types=1);

namespace App;

/** A tree of objects deep and wide enough to test variable trees. */
final class Graph
{
    /** @var list<Node> */
    public array $roots = [];

    public int $nodes = 0;

    public static function build(int $depth, int $width): self
    {
        $graph = new self();
        for ($i = 0; $i < $width; $i++) {
            $graph->roots[] = $graph->node("root-$i", $depth, $width, null);
        }

        return $graph;
    }

    private function node(string $name, int $depth, int $width, ?Node $parent): Node
    {
        $this->nodes++;
        $node = new Node($name, $parent);
        if ($depth > 1) {
            for ($i = 0; $i < $width; $i++) {
                $node->children[] = $this->node("$name.$i", $depth - 1, $width, $node);
            }
        }

        return $node;
    }

    public function summary(): array
    {
        return ['roots' => count($this->roots), 'nodes' => $this->nodes];
    }
}
