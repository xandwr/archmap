/// Embedded web assets for the graph visualization

pub const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Archmap - Dependency Graph</title>
    <script src="https://d3js.org/d3.v7.min.js"></script>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            background: #1a1a2e;
            color: #eee;
            overflow: hidden;
        }

        #container {
            display: flex;
            height: 100vh;
        }

        #graph {
            flex: 1;
            background: #16213e;
        }

        #sidebar {
            width: 320px;
            background: #1a1a2e;
            border-left: 1px solid #333;
            padding: 20px;
            overflow-y: auto;
        }

        h1 {
            font-size: 1.4em;
            margin-bottom: 10px;
            color: #00d9ff;
        }

        h2 {
            font-size: 1.1em;
            margin: 15px 0 10px;
            color: #888;
            text-transform: uppercase;
            letter-spacing: 1px;
        }

        .stat {
            display: flex;
            justify-content: space-between;
            padding: 8px 0;
            border-bottom: 1px solid #333;
        }

        .stat-value {
            color: #00d9ff;
            font-weight: bold;
        }

        #node-info {
            display: none;
            margin-top: 20px;
            padding: 15px;
            background: #16213e;
            border-radius: 8px;
        }

        #node-info.visible {
            display: block;
        }

        #node-info h3 {
            color: #00d9ff;
            margin-bottom: 10px;
            word-break: break-all;
        }

        .node-stat {
            display: flex;
            justify-content: space-between;
            padding: 5px 0;
            font-size: 0.9em;
        }

        .exports-list {
            margin-top: 10px;
            font-size: 0.85em;
        }

        .exports-list span {
            display: inline-block;
            background: #333;
            padding: 2px 8px;
            border-radius: 4px;
            margin: 2px;
        }

        .legend {
            display: flex;
            flex-wrap: wrap;
            gap: 10px;
            margin-top: 15px;
        }

        .legend-item {
            display: flex;
            align-items: center;
            gap: 5px;
            font-size: 0.85em;
        }

        .legend-color {
            width: 12px;
            height: 12px;
            border-radius: 50%;
        }

        .controls {
            margin-top: 20px;
            display: flex;
            flex-direction: column;
            gap: 10px;
        }

        .controls label {
            display: flex;
            align-items: center;
            gap: 8px;
            font-size: 0.9em;
            cursor: pointer;
        }

        .controls input[type="checkbox"] {
            width: 16px;
            height: 16px;
        }

        .controls input[type="range"] {
            flex: 1;
        }

        /* SVG styles */
        .node {
            cursor: pointer;
        }

        .node circle {
            stroke: #fff;
            stroke-width: 1.5px;
        }

        .node text {
            font-size: 10px;
            fill: #fff;
            pointer-events: none;
        }

        .node.highlighted circle {
            stroke: #00d9ff;
            stroke-width: 3px;
        }

        .link {
            stroke: #555;
            stroke-opacity: 0.6;
        }

        .link.cycle {
            stroke: #ff4444;
            stroke-width: 2px;
            stroke-dasharray: 5, 5;
        }

        .link.highlighted {
            stroke: #00d9ff;
            stroke-opacity: 1;
        }

        /* Tooltip */
        .tooltip {
            position: absolute;
            background: rgba(0, 0, 0, 0.9);
            color: #fff;
            padding: 10px;
            border-radius: 6px;
            font-size: 12px;
            pointer-events: none;
            max-width: 250px;
            z-index: 1000;
        }
    </style>
</head>
<body>
    <div id="container">
        <div id="graph"></div>
        <div id="sidebar">
            <h1>Archmap</h1>
            <div id="project-name"></div>

            <h2>Summary</h2>
            <div id="stats">
                <div class="stat">
                    <span>Modules</span>
                    <span class="stat-value" id="stat-modules">-</span>
                </div>
                <div class="stat">
                    <span>Dependencies</span>
                    <span class="stat-value" id="stat-deps">-</span>
                </div>
                <div class="stat">
                    <span>Issues</span>
                    <span class="stat-value" id="stat-issues">-</span>
                </div>
                <div class="stat">
                    <span>Cycles</span>
                    <span class="stat-value" id="stat-cycles">-</span>
                </div>
            </div>

            <h2>Legend</h2>
            <div class="legend">
                <div class="legend-item">
                    <div class="legend-color" style="background: #4ecdc4"></div>
                    <span>Index/Lib</span>
                </div>
                <div class="legend-item">
                    <div class="legend-color" style="background: #ff6b6b"></div>
                    <span>Entry</span>
                </div>
                <div class="legend-item">
                    <div class="legend-color" style="background: #ffe66d"></div>
                    <span>Config</span>
                </div>
                <div class="legend-item">
                    <div class="legend-color" style="background: #c9b1ff"></div>
                    <span>Model</span>
                </div>
                <div class="legend-item">
                    <div class="legend-color" style="background: #95e1d3"></div>
                    <span>Analysis</span>
                </div>
                <div class="legend-item">
                    <div class="legend-color" style="background: #f38181"></div>
                    <span>Parser</span>
                </div>
                <div class="legend-item">
                    <div class="legend-color" style="background: #6c5ce7"></div>
                    <span>Output</span>
                </div>
                <div class="legend-item">
                    <div class="legend-color" style="background: #74b9ff"></div>
                    <span>Module</span>
                </div>
            </div>

            <h2>Controls</h2>
            <div class="controls">
                <label>
                    <input type="checkbox" id="show-labels" checked>
                    Show labels
                </label>
                <label>
                    <input type="checkbox" id="highlight-cycles">
                    Highlight cycles
                </label>
                <label>
                    Node size
                    <input type="range" id="node-scale" min="0.5" max="2" step="0.1" value="1">
                </label>
            </div>

            <div id="node-info">
                <h3 id="node-name"></h3>
                <div class="node-stat">
                    <span>Lines</span>
                    <span id="node-lines">-</span>
                </div>
                <div class="node-stat">
                    <span>Fan-in (dependents)</span>
                    <span id="node-fan-in">-</span>
                </div>
                <div class="node-stat">
                    <span>Fan-out (dependencies)</span>
                    <span id="node-fan-out">-</span>
                </div>
                <div class="node-stat">
                    <span>Issues</span>
                    <span id="node-issues">-</span>
                </div>
                <div class="exports-list">
                    <strong>Exports:</strong>
                    <div id="node-exports"></div>
                </div>
            </div>
        </div>
    </div>

    <div class="tooltip" style="display: none;"></div>

    <script>
        const categoryColors = {
            'index': '#4ecdc4',
            'entry': '#ff6b6b',
            'config': '#ffe66d',
            'model': '#c9b1ff',
            'analysis': '#95e1d3',
            'parser': '#f38181',
            'output': '#6c5ce7',
            'cli': '#fdcb6e',
            'test': '#a29bfe',
            'module': '#74b9ff'
        };

        let simulation, svg, g, link, node, label;
        let graphData;
        let nodeScale = 1;

        async function init() {
            const response = await fetch('/api/graph');
            graphData = await response.json();

            // Update stats
            document.getElementById('project-name').textContent = graphData.metadata.project_name;
            document.getElementById('stat-modules').textContent = graphData.metadata.total_modules;
            document.getElementById('stat-deps').textContent = graphData.metadata.total_dependencies;
            document.getElementById('stat-issues').textContent = graphData.metadata.total_issues;
            document.getElementById('stat-cycles').textContent = graphData.metadata.cycle_count;

            createGraph();
            setupControls();
        }

        function createGraph() {
            const container = document.getElementById('graph');
            const width = container.clientWidth;
            const height = container.clientHeight;

            svg = d3.select('#graph')
                .append('svg')
                .attr('width', width)
                .attr('height', height);

            // Add zoom behavior
            const zoom = d3.zoom()
                .scaleExtent([0.1, 4])
                .on('zoom', (event) => {
                    g.attr('transform', event.transform);
                });

            svg.call(zoom);

            g = svg.append('g');

            // Arrow marker for directed edges
            svg.append('defs').append('marker')
                .attr('id', 'arrowhead')
                .attr('viewBox', '-0 -5 10 10')
                .attr('refX', 20)
                .attr('refY', 0)
                .attr('orient', 'auto')
                .attr('markerWidth', 6)
                .attr('markerHeight', 6)
                .append('path')
                .attr('d', 'M 0,-5 L 10,0 L 0,5')
                .attr('fill', '#555');

            // Links
            link = g.append('g')
                .selectAll('line')
                .data(graphData.links)
                .enter()
                .append('line')
                .attr('class', d => d.is_cycle ? 'link cycle' : 'link')
                .attr('marker-end', 'url(#arrowhead)');

            // Nodes
            node = g.append('g')
                .selectAll('.node')
                .data(graphData.nodes)
                .enter()
                .append('g')
                .attr('class', 'node')
                .call(d3.drag()
                    .on('start', dragstarted)
                    .on('drag', dragged)
                    .on('end', dragended));

            node.append('circle')
                .attr('r', d => getNodeRadius(d))
                .attr('fill', d => categoryColors[d.category] || '#74b9ff');

            // Labels
            label = node.append('text')
                .attr('dy', -12)
                .attr('text-anchor', 'middle')
                .text(d => d.name);

            // Tooltip and click handlers
            const tooltip = d3.select('.tooltip');

            node.on('mouseover', function(event, d) {
                tooltip.style('display', 'block')
                    .html(`<strong>${d.name}</strong><br>
                           ${d.path}<br>
                           Lines: ${d.lines}<br>
                           Fan-in: ${d.fan_in} | Fan-out: ${d.fan_out}`)
                    .style('left', (event.pageX + 10) + 'px')
                    .style('top', (event.pageY - 10) + 'px');

                highlightConnections(d);
            })
            .on('mouseout', function() {
                tooltip.style('display', 'none');
                clearHighlights();
            })
            .on('click', function(event, d) {
                showNodeInfo(d);
            });

            // Force simulation
            simulation = d3.forceSimulation(graphData.nodes)
                .force('link', d3.forceLink(graphData.links)
                    .id(d => d.id)
                    .distance(100))
                .force('charge', d3.forceManyBody().strength(-300))
                .force('center', d3.forceCenter(width / 2, height / 2))
                .force('collision', d3.forceCollide().radius(d => getNodeRadius(d) + 5))
                .on('tick', ticked);
        }

        function getNodeRadius(d) {
            const base = Math.sqrt(d.lines) / 2 + 5;
            return Math.min(Math.max(base, 8), 30) * nodeScale;
        }

        function ticked() {
            link
                .attr('x1', d => d.source.x)
                .attr('y1', d => d.source.y)
                .attr('x2', d => d.target.x)
                .attr('y2', d => d.target.y);

            node.attr('transform', d => `translate(${d.x},${d.y})`);
        }

        function dragstarted(event) {
            if (!event.active) simulation.alphaTarget(0.3).restart();
            event.subject.fx = event.subject.x;
            event.subject.fy = event.subject.y;
        }

        function dragged(event) {
            event.subject.fx = event.x;
            event.subject.fy = event.y;
        }

        function dragended(event) {
            if (!event.active) simulation.alphaTarget(0);
            event.subject.fx = null;
            event.subject.fy = null;
        }

        function highlightConnections(d) {
            const connected = new Set();
            connected.add(d.id);

            link.each(function(l) {
                if (l.source.id === d.id || l.target.id === d.id) {
                    connected.add(l.source.id);
                    connected.add(l.target.id);
                    d3.select(this).classed('highlighted', true);
                }
            });

            node.classed('highlighted', n => connected.has(n.id));
        }

        function clearHighlights() {
            link.classed('highlighted', false);
            node.classed('highlighted', false);
        }

        function showNodeInfo(d) {
            document.getElementById('node-info').classList.add('visible');
            document.getElementById('node-name').textContent = d.path;
            document.getElementById('node-lines').textContent = d.lines;
            document.getElementById('node-fan-in').textContent = d.fan_in;
            document.getElementById('node-fan-out').textContent = d.fan_out;
            document.getElementById('node-issues').textContent = d.issue_count;

            const exportsDiv = document.getElementById('node-exports');
            if (d.exports && d.exports.length > 0) {
                exportsDiv.innerHTML = d.exports.map(e => `<span>${e}</span>`).join('');
            } else {
                exportsDiv.innerHTML = '<em>None</em>';
            }
        }

        function setupControls() {
            document.getElementById('show-labels').addEventListener('change', function() {
                label.style('display', this.checked ? 'block' : 'none');
            });

            document.getElementById('highlight-cycles').addEventListener('change', function() {
                if (this.checked) {
                    link.filter(d => d.is_cycle).style('stroke', '#ff4444').style('stroke-width', 3);
                } else {
                    link.filter(d => d.is_cycle).style('stroke', '#ff4444').style('stroke-width', 2);
                }
            });

            document.getElementById('node-scale').addEventListener('input', function() {
                nodeScale = parseFloat(this.value);
                node.selectAll('circle').attr('r', d => getNodeRadius(d));
                simulation.force('collision', d3.forceCollide().radius(d => getNodeRadius(d) + 5));
                simulation.alpha(0.3).restart();
            });
        }

        // Handle window resize
        window.addEventListener('resize', () => {
            const container = document.getElementById('graph');
            svg.attr('width', container.clientWidth).attr('height', container.clientHeight);
            simulation.force('center', d3.forceCenter(container.clientWidth / 2, container.clientHeight / 2));
            simulation.alpha(0.3).restart();
        });

        // Server-Sent Events for live updates (watch mode)
        function setupSSE() {
            const evtSource = new EventSource('/api/events');

            evtSource.addEventListener('update', async (event) => {
                console.log('Graph update received, version:', event.data);

                // Fetch new graph data
                const response = await fetch('/api/graph');
                const newData = await response.json();

                // Update stats
                document.getElementById('stat-modules').textContent = newData.metadata.total_modules;
                document.getElementById('stat-deps').textContent = newData.metadata.total_dependencies;
                document.getElementById('stat-issues').textContent = newData.metadata.total_issues;
                document.getElementById('stat-cycles').textContent = newData.metadata.cycle_count;

                // Preserve node positions where possible
                const oldPositions = {};
                if (graphData && graphData.nodes) {
                    graphData.nodes.forEach(n => {
                        oldPositions[n.id] = { x: n.x, y: n.y, vx: n.vx, vy: n.vy };
                    });
                }

                // Apply old positions to new nodes
                newData.nodes.forEach(n => {
                    if (oldPositions[n.id]) {
                        n.x = oldPositions[n.id].x;
                        n.y = oldPositions[n.id].y;
                        n.vx = oldPositions[n.id].vx;
                        n.vy = oldPositions[n.id].vy;
                    }
                });

                graphData = newData;

                // Update links
                link = link.data(graphData.links, d => `${d.source.id || d.source}-${d.target.id || d.target}`);
                link.exit().remove();
                link = link.enter()
                    .append('line')
                    .attr('class', d => d.is_cycle ? 'link cycle' : 'link')
                    .attr('marker-end', 'url(#arrowhead)')
                    .merge(link);

                // Update nodes
                node = node.data(graphData.nodes, d => d.id);
                node.exit().remove();
                const nodeEnter = node.enter()
                    .append('g')
                    .attr('class', 'node')
                    .call(d3.drag()
                        .on('start', dragstarted)
                        .on('drag', dragged)
                        .on('end', dragended));

                nodeEnter.append('circle')
                    .attr('r', d => getNodeRadius(d))
                    .attr('fill', d => categoryColors[d.category] || '#74b9ff');

                nodeEnter.append('text')
                    .attr('dy', -12)
                    .attr('text-anchor', 'middle')
                    .text(d => d.name);

                const tooltip = d3.select('.tooltip');
                nodeEnter.on('mouseover', function(event, d) {
                    tooltip.style('display', 'block')
                        .html(`<strong>${d.name}</strong><br>${d.path}<br>Lines: ${d.lines}<br>Fan-in: ${d.fan_in} | Fan-out: ${d.fan_out}`)
                        .style('left', (event.pageX + 10) + 'px')
                        .style('top', (event.pageY - 10) + 'px');
                    highlightConnections(d);
                })
                .on('mouseout', function() {
                    tooltip.style('display', 'none');
                    clearHighlights();
                })
                .on('click', function(event, d) {
                    showNodeInfo(d);
                });

                node = nodeEnter.merge(node);

                // Update existing node visuals
                node.select('circle')
                    .attr('r', d => getNodeRadius(d))
                    .attr('fill', d => categoryColors[d.category] || '#74b9ff');
                node.select('text').text(d => d.name);

                label = node.selectAll('text');

                // Restart simulation with new data
                simulation.nodes(graphData.nodes);
                simulation.force('link').links(graphData.links);
                simulation.alpha(0.3).restart();

                // Flash indicator
                const indicator = document.createElement('div');
                indicator.style.cssText = 'position:fixed;top:10px;left:50%;transform:translateX(-50%);background:#00d9ff;color:#000;padding:8px 16px;border-radius:4px;font-weight:bold;z-index:9999;';
                indicator.textContent = 'Graph Updated';
                document.body.appendChild(indicator);
                setTimeout(() => indicator.remove(), 2000);
            });

            evtSource.onerror = () => {
                console.log('SSE connection lost, reconnecting...');
            };
        }

        init();
        setupSSE();
    </script>
</body>
</html>
"#;
