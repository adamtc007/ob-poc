//! Visitor pattern implementations for AST traversal

use super::*;

#[allow(dead_code)]
pub(crate) trait StatementVisitor {
    fn visit_declare_entity(&mut self, entity: &DeclareEntity);
    fn visit_obtain_document(&mut self, doc: &ObtainDocument);
    fn visit_create_edge(&mut self, edge: &CreateEdge);
    fn visit_calculate_ubo(&mut self, calc: &CalculateUbo);
}

#[allow(dead_code)]
pub(crate) trait AstWalker {
    fn walk_program(&mut self, program: &Program) {
        for workflow in &program.workflows {
            self.walk_workflow(workflow);
        }
    }

    fn walk_workflow(&mut self, workflow: &Workflow) {
        for statement in &workflow.statements {
            self.walk_statement(statement);
        }
    }

    fn walk_statement(&mut self, statement: &Statement);
}
