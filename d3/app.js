var svg = d3.select("svg"),
    width = +svg.attr("width"),
    height = +svg.attr("height");

var rect = svg.append("rect")
    .attr("width", "100%")
    .attr("height", "100%")
    .attr("fill", "black");

var data = { "vertices": [], "edges": [] };
var nodes = data["vertices"];
var links = data["edges"];

const simulation = d3.forceSimulation().on("tick", ticked);

var link = svg.append("g")
    .attr("stroke", "#999")
    .attr("stroke-opacity", 0.6)
    .selectAll("line");

var node = svg.append("g")
    .attr("stroke", "#fff")
    .attr("stroke-width", 1.5)
    .selectAll("circle");

let zoom = d3.zoom()
    .on("zoom", zoomed);


svg.call(zoom);

// Stores transform state, useful for applying zoom/pan to newly added elements.
var t;

function zoomed({ transform }) {
    t = transform;
    link.attr("transform", transform);
    node.attr("transform", transform);
}

restart();

function restart() {
    d3.timeout(function() {

        postData('http://localhost:3030', { "jsonrpc": "2.0", "method": "getnetworkgraph", "id": 123 })
            .then(data => {
                // console.log(JSON.stringify(data));
                merge_data(data);
                update_graph();
                restart();
            });
    }, 3000);
}

function merge_data(data) {

    var vertices = data["result"]["vertices"];
    var edges = data["result"]["edges"];

    // Calculate the diffs.
    let added_nodes = vertices.filter(x => {
        return !nodes.some(node => node.id == x.id)
    });
    let removed_nodes = nodes.filter(x => {
        return !vertices.some(node => node.id == x.id)
    });

    // console.log(added_nodes.length);

    let added_links = edges.filter(x => {
        return !links.some(link => link.source == x.source && link.target == x.target)
    });
    let removed_links = links.filter(x => {
        return !edges.some(link => link.source == x.source && link.target == x.target)
    });

    removed_nodes.forEach(removed_node => {
        const isRemoved = (node) => node.id == removed_node.id;
        var i = nodes.findIndex(isRemoved);
        nodes.splice(i, 1);
    });

    added_nodes.forEach(added_node => {
        nodes.push(added_node)
    });

    removed_links.forEach(removed_link => {
        const isRemoved = (link) => link.source.id == removed_link.source && link.target.id == removed_link.target;
        var i = links.findIndex(isRemoved);
        links.splice(i, 1);
    });

    added_links.forEach(added_link => {
        links.push(added_link)
    });
}

function update_graph() {
    // Apply the general update pattern to the nodes including zoom/pan.
    node = node.data(nodes, d => d.id).join(
        enter => enter.append("circle").attr("fill", d => {
            if (d.is_bootnode) { return "red" }
        }).attr("r", 5).attr("transform", t),
        update => update,
        exit => exit.remove()
    );

    node.append("title").text(function(d) { return d.id })

    link = link.data(links, d => d.source.id + "-" + d.target.id).join(
        enter => enter.append("line")
        .attr("stroke-width", .8).attr("transform", t),
        update => update,
        exit => exit.remove()
    );

    // Update and restart the simulation.
    simulation.nodes(nodes);
    simulation.force("link", d3.forceLink(links).id(d => d.id).distance(50))
        .force("charge", d3.forceManyBody().strength(-100))
        // .force("center", d3.forceCenter(width / 2, height / 2));
        .force("x", d3.forceX(width / 2).strength(.05))
        .force("y", d3.forceY(height / 2).strength(.05));
    simulation.alpha(.7).restart();
}

function ticked() {
    link
        .attr("x1", d => d.source.x)
        .attr("y1", d => d.source.y)
        .attr("x2", d => d.target.x)
        .attr("y2", d => d.target.y);

    node
        .attr("cx", d => d.x)
        .attr("cy", d => d.y);
}

async function postData(url = '', data = {}) {
    const response = await fetch(url, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json'
        },
        body: JSON.stringify(data)
    });
    return response.json();
}